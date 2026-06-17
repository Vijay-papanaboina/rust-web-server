use crate::server::jwt::Jwt;
use http::Request;
use http::response::{Response, StatusCode};
use std::error::Error;

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
        response: &mut Response,
    ) -> Result<(), Box<dyn Error>> {
        let auth_header = match request.headers.get("Authorization") {
            Some(header) => header,
            None => {
                let response_body = r#"{"error": "Authorization header is missing"}"#;
                response.status(StatusCode::UNAUTHORIZED);
                response.headers.set("Content-Type", "application/json");
                response.send(response_body.as_bytes()).await?;
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
                let response_body = r#"{"error": "Invalid token"}"#;
                response.status(StatusCode::UNAUTHORIZED);
                response.headers.set("Content-Type", "application/json");
                response.send(response_body.as_bytes()).await?;
                return Err(format!("Invalid token: {}", err_msg).into());
            }
        };

        request.extensions.insert(claims);
        Ok(())
    }
}

pub fn logger(request: &Request) {
    println!("Request-Line: {} {}", request.method, request.path);
    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&request.body) {
        println!("Request-Body:{json:#}\n");
    }
}
