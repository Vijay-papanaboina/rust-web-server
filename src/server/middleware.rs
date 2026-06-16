use crate::server::jwt::Jwt;
use crate::server::request::Request;
use crate::server::response::{StatusCode, send_response};
use std::error::Error;
use tokio::net::TcpStream;

pub struct Middleware {
    pub jwt: Jwt,
}

impl Middleware {
    pub fn new(jwt: Jwt) -> Self {
        Self { jwt }
    }
    pub async fn check_auth(
        &self,
        request: &mut Request,
        stream: &mut TcpStream,
    ) -> Result<(), Box<dyn Error>> {
        let auth_header = match request.headers.get("Authorization") {
            Some(header) => header,
            None => {
                let response = r#"{"error": "Authorization header is missing"}"#;
                let _ = send_response(
                    stream,
                    StatusCode::Unauthorized,
                    "application/json",
                    response.as_bytes(),
                )
                .await;
                return Err("Authorization header is missing".into());
            }
        };
        let token = auth_header.trim_start_matches("Bearer ");

        // Map the non-Send `Box<dyn Error>` to `String` synchronously before any `.await` points
        // to prevent compiler errors from holding a non-Send type across thread-spawning task boundaries.
        let claims = match self.jwt.decode(token).map_err(|e| e.to_string()) {
            Ok(claims) => claims,
            Err(err_msg) => {
                eprintln!("Error decoding token: {}", err_msg);
                let response = r#"{"error": "Invalid token"}"#;
                let _ = send_response(
                    stream,
                    StatusCode::Unauthorized,
                    "application/json",
                    response.as_bytes(),
                )
                .await;
                return Err(format!("Invalid token: {}", err_msg).into());
            }
        };

        request.user = Some(claims);
        Ok(())
    }
}

pub fn logger(request: &Request) {
    println!("Request-Line: {} {}", request.method, request.path);
    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&request.body) {
        println!("Request-Body:{json:#}\n");
    }
}
