use serde::{self, Deserialize, Serialize};
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::net::TcpStream;

mod mroutes;
mod middleware;

#[derive( Serialize, Deserialize)]
pub struct Request {
    method: String,
    path: String,
    version: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let body_decoded = self.json::<serde_json::Value>().ok().unwrap_or_default();
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("path", &self.path)
            .field("version", &self.version)
            .field("headers", &self.headers)
            .field("body_decoded", &body_decoded)
            .field("body", &self.body)
            .finish()
    }
}
impl Request {
    #[allow(dead_code)]
    pub fn json<T>(&self) -> serde_json::Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        serde_json::from_slice(&self.body)
    }
}

pub async fn handle_request(stream: &mut TcpStream) {
    if let Some(request) = parse_request(stream).await {
    middleware::logger(&request);
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => mroutes::index(&request, stream).await,
        ("POST", "/echo") => mroutes::echo(&request, stream).await,
        // ("POST", "/json") => mroutes::json(request, stream).await,
        _ => mroutes::not_found(stream).await,
    }
    } else {
        mroutes::not_found(stream).await;
    }
}

async fn parse_request(stream: &mut TcpStream) -> Option<Request> {
    let buf_reader = BufReader::new(stream);
    let mut lines = buf_reader.lines();
    let mut http_request = Vec::new();
    while let Some(line) = lines.next_line().await.ok() {
        let line = line?;
        if line.is_empty() {
            break;
        }
        http_request.push(line);
    }
    if http_request.is_empty() {
        println!("Malformed Request!");
        return None;
    }
    let request_line = &http_request[0];
    let mut parts = request_line.split_whitespace();

    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let version = parts.next()?.to_string();
    let headers: HashMap<String, String> = http_request
        .iter()
        .skip(1)
        .filter_map(|header| {
            header
                .split_once(':')
                .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        })
        .collect();
    let content_length = headers
        .get("Content-Length")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);

    let mut body = vec![0; content_length];

    if content_length > 0 {
        let mut buf_reader = lines.into_inner();
        buf_reader.read_exact(&mut body).await.ok()?;
    }
    return Some(Request {
        method,
        path,
        version,
        headers,
        body,
    });
}
