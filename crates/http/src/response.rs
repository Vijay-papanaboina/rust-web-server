use std::collections::HashMap;
use std::error::Error;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StatusCode(pub u16);

impl StatusCode {
    pub const OK: StatusCode = StatusCode(200);
    pub const CREATED: StatusCode = StatusCode(201);
    pub const BAD_REQUEST: StatusCode = StatusCode(400);
    pub const UNAUTHORIZED: StatusCode = StatusCode(401);
    pub const NOT_FOUND: StatusCode = StatusCode(404);
    pub const INTERNAL_SERVER_ERROR: StatusCode = StatusCode(500);

    pub fn as_u16(&self) -> u16 {
        self.0
    }

    pub fn canonical_reason(&self) -> Option<&'static str> {
        match self.0 {
            200 => Some("OK"),
            201 => Some("Created"),
            400 => Some("Bad Request"),
            401 => Some("Unauthorized"),
            404 => Some("Not Found"),
            500 => Some("Internal Server Error"),
            _ => None,
        }
    }

    pub fn as_str(&self) -> String {
        match self.canonical_reason() {
            Some(reason) => format!("{} {}", self.0, reason),
            None => self.0.to_string(),
        }
    }
}

pub struct Headers {
    pub map: HashMap<String, String>,
}

impl Headers {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.map
            .insert(key.trim().to_string(), value.trim().to_string());
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.map.get(key)
    }
}

pub struct Response {
    stream: TcpStream,
    pub status_code: StatusCode,
    pub headers: Headers,
}

impl Response {
    pub fn new(stream: TcpStream) -> Self {
        let mut headers = Headers::new();
        headers.set("Server", "RustWebServer/0.1");

        Self {
            stream,
            status_code: StatusCode::OK,
            headers,
        }
    }

    pub fn status(&mut self, status: StatusCode) -> &mut Self {
        self.status_code = status;
        self
    }

    pub fn into_stream(self) -> TcpStream {
        self.stream
    }

    pub fn stream_mut(&mut self) -> &mut TcpStream {
        &mut self.stream
    }

    pub async fn send(&mut self, body: &[u8]) -> Result<(), Box<dyn Error>> {
        if self.headers.get("Content-Length").is_none() {
            self.headers.set("Content-Length", &body.len().to_string());
        }

        let mut response_bytes = format!("HTTP/1.1 {}\r\n", self.status_code.as_str()).into_bytes();
        for (key, value) in &self.headers.map {
            response_bytes.extend_from_slice(format!("{}: {}\r\n", key, value).as_bytes());
        }
        response_bytes.extend_from_slice(b"\r\n");
        response_bytes.extend_from_slice(body);

        self.stream.write_all(&response_bytes).await?;
        Ok(())
    }
}
