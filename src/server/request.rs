use serde::{self, Deserialize, Serialize};
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::net::TcpStream;

use crate::server::jwt::Claims;
use crate::server::middleware;
use crate::server::routes;
use crate::server::routes::Controller;
use std::error::Error;

#[derive(Serialize, Deserialize)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub user: Option<Claims>,
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

    pub async fn parse_json<T>(&self, stream: &mut TcpStream) -> Option<T>
    where
        T: serde::de::DeserializeOwned,
    {
        match self.json::<T>() {
            Ok(data) => Some(data),
            Err(_) => {
                let response = r#"{"error": "Invalid JSON payload"}"#;
                let _ = crate::server::response::send_response(
                    stream,
                    crate::server::response::StatusCode::BadRequest,
                    "application/json",
                    response.as_bytes(),
                )
                .await;
                None
            }
        }
    }
}

pub async fn handle_request(controller: &Controller, mut stream: TcpStream) -> Result<(), Box<dyn Error>> {
    if let Some(mut request) = parse_request(&mut stream).await {
        middleware::logger(&request);
        routes::route(controller, &mut request, stream).await?;
    } else {
        routes::not_found(&mut stream).await?;
    }
    Ok(())
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
        user: None,
    });
}
