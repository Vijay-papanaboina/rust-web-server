use std::collections::HashMap;
use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};

use base64::{Engine, engine::general_purpose as b64};
use sha1::{Digest, Sha1};
use tokio::sync::mpsc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
};

use crate::server::request::Request;

static SOCKETS: LazyLock<Mutex<HashMap<usize, mpsc::UnboundedSender<Vec<u8>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT_SOCKET_ID: AtomicUsize = AtomicUsize::new(1);
static CHATS: LazyLock<Mutex<HashMap<usize, Vec<usize>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static USERS: LazyLock<Mutex<HashMap<usize, usize>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn make_ws_frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::new();
    let first_byte = 0x80 | (opcode & 0x0F);

    if payload.len() <= 125 {
        frame.push(first_byte);
        frame.push(payload.len() as u8);
        frame.extend_from_slice(&payload);
    } else if payload.len() <= 65535 {
        frame.push(first_byte);
        frame.push(126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        frame.extend_from_slice(&payload);
    } else {
        frame.push(first_byte);
        frame.push(127);
        frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        frame.extend_from_slice(&payload);
    }
    frame
}

pub fn broadcast(opcode: u8, payload: &[u8], chat_id: usize, sender_id: usize) {
    let frame = make_ws_frame(opcode, payload);
    let sockets = SOCKETS.lock().unwrap();
    let chats = CHATS.lock().unwrap();
    if let Some(chat_members) = chats.get(&chat_id) {
        for (&id, tx) in sockets.iter() {
            if chat_members.contains(&id) && id != sender_id {
                tx.send(frame.clone()).ok();
            }
        }
    }
}

pub async fn handle_ws(request: &Request, mut stream: TcpStream) -> Result<(), Box<dyn Error>> {
    let chat_id = request
        .query_params
        .get("chat_id")
        .ok_or("Missing chat_id parameter")?
        .parse::<usize>()?;
    let user_id = request
        .query_params
        .get("user_id")
        .ok_or("Missing user_id parameter")?
        .parse::<usize>()?;
    upgrade_ws(request, &mut stream).await?;
    let (mut stream_read, stream_write) = stream.into_split();
    let (client_id, tx) = register_client(stream_write, chat_id, user_id);

    let mut message_buffer: Vec<u8> = Vec::new();
    let mut message_opcode: u8 = 0;
    loop {
        let mut header = [0u8; 2];
        if read_exact_or_eof(&mut stream_read, &mut header).await? {
            break;
        }
        let (is_fin, opcode, has_mask, payload_len) = parse_frame_header(&header);

        if !has_mask {
            // Section 5.1: Close the connection if a client sends an unmasked frame
            println!("Client sent an unmasked frame. Disconnecting.");
            break;
        }

        if opcode != 0 && opcode < 8 && opcode != message_opcode {
            message_opcode = opcode;
        }

        let actual_payload_len = get_payload_len(payload_len, &mut stream_read).await?;

        // Read the 4-byte masking key
        let mut mask_key = [0u8; 4];
        if read_exact_or_eof(&mut stream_read, &mut mask_key).await? {
            break;
        }

        // Read the payload
        let mut payload = vec![0u8; actual_payload_len as usize];
        if read_exact_or_eof(&mut stream_read, &mut payload).await? {
            break;
        }

        // Unmask the payload
        for i in 0..payload.len() {
            payload[i] ^= mask_key[i % 4];
        }

        // Handle Control Frames (opcode >= 8)
        if opcode >= 8 {
            match opcode {
                8 => {
                    println!("Close frame");
                    break;
                }
                9 => {
                    println!("Received Ping, replying with Pong");
                    let pong_frame = make_ws_frame(10, &payload);
                    tx.send(pong_frame).ok();
                }
                10 => {
                    println!("Received Pong");
                }
                _ => {
                    println!("Unknown control frame: {}", opcode);
                }
            }
            continue;
        }

        // Handle Data Frames (opcode < 8)
        message_buffer.extend_from_slice(&payload);

        if is_fin {
            match message_opcode {
                0 => {
                    println!("Continuation frame");
                }
                1 => {
                    println!("Text frame");
                    if let Ok(text) = String::from_utf8(message_buffer.clone()) {
                        println!("Received text: {}", text);
                        broadcast(1, &message_buffer, chat_id, client_id);
                    }
                }
                2 => {
                    println!("Received binary of length {}", message_buffer.len());
                    broadcast(2, &message_buffer, chat_id, client_id);
                }
                _ => {
                    println!("Unknown opcode");
                }
            }

            message_opcode = 0;
            message_buffer = Vec::new();
        }
    }
    unregister_client(client_id);
    Ok(())
}

async fn upgrade_ws(request: &Request, stream: &mut TcpStream) -> Result<(), Box<dyn Error>> {
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
    println!("is_fin: {}", is_fin);
    println!("opcode: {}", opcode);
    println!("has_mask: {}", has_mask);
    println!("payload_len: {}\n", payload_len);
    (is_fin, opcode, has_mask, payload_len as u64)
}

async fn read_exact_or_eof(
    stream: &mut OwnedReadHalf,
    buf: &mut [u8],
) -> Result<bool, Box<dyn Error>> {
    match stream.read_exact(buf).await {
        Ok(_) => Ok(false),
        Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(true),
        Err(e) => Err(e.into()),
    }
}

async fn get_payload_len(
    payload_len: u64,
    stream: &mut OwnedReadHalf,
) -> Result<u64, Box<dyn Error>> {
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

fn register_client(
    stream_write: OwnedWriteHalf,
    chat_id: usize,
    user_id: usize,
) -> (usize, mpsc::UnboundedSender<Vec<u8>>) {
    let socket_id = NEXT_SOCKET_ID.fetch_add(1, Ordering::Relaxed);
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();

    // Spawn the writer task to handle writing out to the socket
    tokio::spawn(async move {
        let mut stream_write = stream_write;
        while let Some(msg) = rx.recv().await {
            if stream_write.write_all(&msg).await.is_err() {
                break;
            }
        }
    });

    SOCKETS.lock().unwrap().insert(socket_id, tx.clone());
    CHATS
        .lock()
        .unwrap()
        .entry(chat_id)
        .or_insert(Vec::new())
        .push(socket_id);
    USERS.lock().unwrap().insert(socket_id, user_id);
    println!("Client {} connected", socket_id);
    (socket_id, tx)
}

fn unregister_client(socket_id: usize) {
    println!("Client {} disconnected", socket_id);
    SOCKETS.lock().unwrap().remove(&socket_id);
    USERS.lock().unwrap().remove(&socket_id);
    CHATS.lock().unwrap().values_mut().for_each(|clients| {
        clients.retain(|&id| id != socket_id);
    });
}
