use serde::{Deserialize, Serialize};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::error::Error;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

pub mod response;

#[derive(Default)]
pub struct Extensions {
    map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl Extensions {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn insert<T: Send + Sync + 'static>(&mut self, val: T) -> Option<T> {
        self.map
            .insert(TypeId::of::<T>(), Box::new(val))
            .and_then(|boxed| boxed.downcast::<T>().ok().map(|boxed| *boxed))
    }

    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref::<T>())
    }

    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.map
            .get_mut(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_mut::<T>())
    }
}

#[derive(Serialize, Deserialize)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query_params: HashMap<String, String>,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    #[serde(skip)]
    pub extensions: Extensions,
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
                let mut response_bytes = Vec::new();
                response_bytes.extend_from_slice(b"HTTP/1.1 400 Bad Request\r\n");
                response_bytes.extend_from_slice(b"Content-Type: application/json\r\n");
                response_bytes.extend_from_slice(
                    format!("Content-Length: {}\r\n", response.len()).as_bytes(),
                );
                response_bytes.extend_from_slice(b"\r\n");
                response_bytes.extend_from_slice(response.as_bytes());
                let _ = stream.write_all(&response_bytes).await;
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
            extensions: Extensions::new(),
        })
    }
}

#[cfg(test)]
mod tests;
