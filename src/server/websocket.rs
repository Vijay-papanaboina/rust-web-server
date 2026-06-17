use std::collections::HashMap;
use std::error::Error;

use base64::{Engine, engine::general_purpose as b64};
use sha1::{Digest, Sha1};
use tokio::sync::mpsc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, tcp::OwnedReadHalf},
};

use http::Request;

#[derive(Debug, Clone)]
pub enum Message {
    Text(String),
    Binary(Vec<u8>),
    #[allow(dead_code)]
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close,
}

pub struct WsSender {
    tx: mpsc::UnboundedSender<Message>,
}

pub struct WsReceiver {
    rx: mpsc::UnboundedReceiver<WsEvent>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Handshake {
    pub query_params: HashMap<String, String>,
    pub headers: HashMap<String, String>,
}

pub enum WsEvent {
    Connect {
        socket_id: usize,
        handshake: Handshake,
        sender: WsSender,
    },
    Message(Message),
    Disconnect,
}

impl WsSender {
    pub fn send(&self, msg: Message) -> Result<(), mpsc::error::SendError<Message>> {
        self.tx.send(msg)
    }
}

impl WsReceiver {
    pub async fn recv(&mut self) -> Option<WsEvent> {
        self.rx.recv().await
    }
}

fn make_ws_frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::new();
    let first_byte = 0x80 | (opcode & 0x0F);

    if payload.len() <= 125 {
        frame.push(first_byte);
        frame.push(payload.len() as u8);
        frame.extend_from_slice(payload);
    } else if payload.len() <= 65535 {
        frame.push(first_byte);
        frame.push(126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        frame.extend_from_slice(payload);
    } else {
        frame.push(first_byte);
        frame.push(127);
        frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        frame.extend_from_slice(payload);
    }
    frame
}

pub async fn upgrade(
    request: &Request,
    mut stream: TcpStream,
) -> Result<WsReceiver, Box<dyn Error + Send + Sync>> {
    static NEXT_SOCKET_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

    upgrade_ws(request, &mut stream).await?;
    let (stream_read, stream_write) = stream.into_split();

    let socket_id = NEXT_SOCKET_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let handshake = Handshake {
        query_params: request.query_params.clone(),
        headers: request.headers.clone(),
    };

    let (tx_out, rx_out) = mpsc::unbounded_channel::<Message>();
    let (tx_in, rx_in) = mpsc::unbounded_channel::<WsEvent>();

    let _ = tx_in.send(WsEvent::Connect {
        socket_id,
        handshake,
        sender: WsSender { tx: tx_out.clone() },
    });

    // Spawn the writer task
    tokio::spawn(async move {
        let mut stream_write = stream_write;
        let mut rx_out = rx_out;
        while let Some(msg) = rx_out.recv().await {
            let frame = match msg {
                Message::Text(ref s) => make_ws_frame(1u8, s.as_bytes()),
                Message::Binary(ref b) => make_ws_frame(2u8, b.as_slice()),
                Message::Ping(ref p) => make_ws_frame(9u8, p.as_slice()),
                Message::Pong(ref p) => make_ws_frame(10u8, p.as_slice()),
                Message::Close => make_ws_frame(8u8, &[]),
            };
            if stream_write.write_all(&frame).await.is_err() {
                break;
            }
        }
    });

    // Spawn the reader task
    let tx_out_clone = tx_out.clone();
    tokio::spawn(async move {
        let mut stream_read = stream_read;
        let mut message_buffer: Vec<u8> = Vec::new();
        let mut message_opcode: u8 = 0;
        let tx_in = tx_in;
        let tx_out = tx_out_clone;

        loop {
            let mut header = [0u8; 2];
            match read_exact_or_eof(&mut stream_read, &mut header).await {
                Ok(true) => break, // EOF
                Ok(false) => {}
                Err(e) => {
                    eprintln!("Error reading header: {}", e);
                    break;
                }
            }
            let (is_fin, opcode, has_mask, payload_len) = parse_frame_header(&header);

            if !has_mask {
                eprintln!("Client sent an unmasked frame. Disconnecting.");
                let _ = tx_out.send(Message::Close);
                break;
            }

            if opcode != 0 && opcode < 8 && opcode != message_opcode {
                message_opcode = opcode;
            }

            let actual_payload_len = match get_payload_len(payload_len, &mut stream_read).await {
                Ok(len) => len,
                Err(e) => {
                    eprintln!("Error getting payload length: {}", e);
                    break;
                }
            };

            let mut mask_key = [0u8; 4];
            match read_exact_or_eof(&mut stream_read, &mut mask_key).await {
                Ok(true) => break, // EOF
                Ok(false) => {}
                Err(e) => {
                    eprintln!("Error reading mask key: {}", e);
                    break;
                }
            }

            // Read the payload
            let mut payload = vec![0u8; actual_payload_len as usize];
            match read_exact_or_eof(&mut stream_read, &mut payload).await {
                Ok(true) => break, // EOF
                Ok(false) => {}
                Err(e) => {
                    eprintln!("Error reading payload: {}", e);
                    break;
                }
            }

            // Unmask the payload
            for i in 0..payload.len() {
                payload[i] ^= mask_key[i % 4];
            }

            // Handle Control Frames (opcode >= 8)
            if opcode >= 8 {
                match opcode {
                    8 => {
                        let _ = tx_out.send(Message::Close);
                        break;
                    }
                    9 => {
                        let _ = tx_out.send(Message::Pong(payload));
                    }
                    10 => {
                        if tx_in
                            .send(WsEvent::Message(Message::Pong(payload)))
                            .is_err()
                        {
                            break;
                        }
                    }
                    _ => {
                        eprintln!("Unknown control frame: {}", opcode);
                    }
                }
                continue;
            }

            // Handle Data Frames (opcode < 8)
            message_buffer.extend_from_slice(&payload);

            if is_fin {
                let msg = match message_opcode {
                    1 => {
                        if let Ok(text) = String::from_utf8(message_buffer.clone()) {
                            Some(Message::Text(text))
                        } else {
                            None
                        }
                    }
                    2 => Some(Message::Binary(message_buffer.clone())),
                    _ => None,
                };

                if let Some(msg) = msg {
                    if tx_in.send(WsEvent::Message(msg)).is_err() {
                        break;
                    }
                }

                message_opcode = 0;
                message_buffer.clear();
            }
        }
        let _ = tx_in.send(WsEvent::Disconnect);
    });

    Ok(WsReceiver { rx: rx_in })
}

async fn upgrade_ws(
    request: &Request,
    stream: &mut TcpStream,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let web_socket_key = request
        .headers
        .get("Sec-WebSocket-Key")
        .ok_or("Sec-WebSocket-Key not found")?;
    let magic_key = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let combined_key = format!("{}{}", web_socket_key, magic_key);
    let web_socket_accept_key = b64::STANDARD.encode(Sha1::digest(combined_key.as_bytes()));

    let handshake_response = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {}\r\n\
         \r\n",
        web_socket_accept_key
    );

    stream.write_all(handshake_response.as_bytes()).await?;
    Ok(())
}

fn parse_frame_header(header: &[u8]) -> (bool, u8, bool, u64) {
    let is_fin = (header[0] >> 7) == 1;
    let opcode = header[0] & 0b00001111;
    let has_mask = (header[1] >> 7) == 1;
    let payload_len = header[1] & 0b01111111;
    (is_fin, opcode, has_mask, payload_len as u64)
}

async fn read_exact_or_eof(
    stream: &mut OwnedReadHalf,
    buf: &mut [u8],
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    match stream.read_exact(buf).await {
        Ok(_) => Ok(false),
        Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(true),
        Err(e) => Err(e.into()),
    }
}

async fn get_payload_len(
    payload_len: u64,
    stream: &mut OwnedReadHalf,
) -> Result<u64, Box<dyn Error + Send + Sync>> {
    if payload_len <= 125 {
        Ok(payload_len)
    } else if payload_len == 126 {
        let mut ext_payload_len = [0u8; 2];
        stream.read_exact(&mut ext_payload_len).await?;
        Ok(u16::from_be_bytes(ext_payload_len) as u64)
    } else if payload_len == 127 {
        let mut ext_payload_len = [0u8; 8];
        stream.read_exact(&mut ext_payload_len).await?;
        Ok(u64::from_be_bytes(ext_payload_len))
    } else {
        Ok(0)
    }
}
