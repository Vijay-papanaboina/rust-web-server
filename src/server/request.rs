use serde::{self, Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::net::TcpStream;

use crate::server::jwt::Claims;

#[derive(Serialize, Deserialize)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query_params: HashMap<String, String>,
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
            .field("query_params", &self.query_params)
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

    pub async fn parse(stream: &mut TcpStream) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let buf_reader = BufReader::new(stream);
        let mut lines = buf_reader.lines();
        let mut http_request = Vec::new();
        while let Some(line) = lines.next_line().await? {
            if line.is_empty() {
                break;
            }
            http_request.push(line);
        }
        if http_request.is_empty() {
            return Err("Empty or malformed HTTP request".into());
        }
        let request_line = &http_request[0];
        let mut parts = request_line.split_whitespace();

        let method = parts.next().ok_or("Missing HTTP method")?.to_string();
        let fullpath = parts.next().ok_or("Missing request path")?.to_string();
        let mut fullpath = fullpath.split('?');
        let path = fullpath.next().ok_or("Invalid path")?.to_string();
        let query_string = fullpath.next().unwrap_or("");
        let query_params: HashMap<String, String> = query_string
            .split('&')
            .filter(|s| !s.is_empty())
            .filter_map(|pair| {
                pair.split_once('=')
                    .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            })
            .collect();
        let version = parts.next().ok_or("Missing HTTP version")?.to_string();
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
            buf_reader.read_exact(&mut body).await?;
        }
        Ok(Request {
            method,
            path,
            query_params,
            version,
            headers,
            body,
            user: None,
        })
    }
}
