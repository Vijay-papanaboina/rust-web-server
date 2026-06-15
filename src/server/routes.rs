use std::error::Error;
use tokio::net::TcpStream;

use crate::server::middleware::Middleware;
use crate::server::models::{CreateAccountRequest, LoginRequest, LoginResponse};
use crate::server::request::Request;
use crate::server::response::{StatusCode, send_response};
use crate::server::services::Service;
use crate::server::jwt::Claims;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn route(
    controller: &Controller,
    request: &mut Request,
    stream: &mut TcpStream,
) -> Result<(), Box<dyn Error>> {
    // Check request path ignoring query parameters for matching routes
    let path = request.path.split('?').next().unwrap_or("/");

    match (request.method.as_str(), path) {
        ("GET", "/") => controller.index(request, stream).await,
        ("POST", "/login") => controller.login(request, stream).await,
        ("POST", "/register") => controller.create_account(request, stream).await,
        ("GET", "/user") => {
            controller.middleware.check_auth(request, stream).await?;
            controller.get_user(request, stream).await
        }
        _ => not_found(stream).await,
    }
}

pub struct Controller {
    pub service: Service,
    pub middleware: Middleware,
}

impl Controller {
    pub fn new(service: Service, middleware: Middleware) -> Self {
        Self {
            service,
            middleware,
        }
    }

    pub async fn index(
        &self,
        request: &Request,
        stream: &mut TcpStream,
    ) -> Result<(), Box<dyn Error>> {
        let body = format!(
            "This is a web server written in Rust without any framework.\n\
             You are on the {} {} path.\n",
            request.method, request.path
        );
        send_response(stream, StatusCode::Ok, "text/plain", body.as_bytes()).await?;
        Ok(())
    }

    pub async fn login(
        &self,
        request: &Request,
        stream: &mut TcpStream,
    ) -> Result<(), Box<dyn Error>> {
        let req_data = match request.parse_json::<LoginRequest>(stream).await {
            Some(data) => data,
            None => return Ok(()),
        };

        let response = self.service.login(req_data.email, req_data.password).await;
        match response {
            Some(user) => {
                let exp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as usize
                    + 24 * 3600; // 24 hours
                let claims = Claims {
                    sub: user.email.clone(),
                    exp,
                };
                let token = match self.middleware.jwt.encode(&claims).map_err(|e| e.to_string()) {
                    Ok(t) => t,
                    Err(err_msg) => {
                        eprintln!("JWT encode error: {}", err_msg);
                        let response = r#"{"error": "Failed to generate token"}"#;
                        send_response(
                            stream,
                            StatusCode::InternalServerError,
                            "application/json",
                            response.as_bytes(),
                        )
                        .await?;
                        return Ok(());
                    }
                };
                let login_response = LoginResponse {
                    id: user.id,
                    username: user.username,
                    email: user.email,
                    token,
                };
                let response_bytes = serde_json::to_vec(&login_response)?;
                send_response(stream, StatusCode::Ok, "application/json", &response_bytes).await?;
            }
            None => {
                let response = r#"{"error": "Invalid credentials"}"#;
                send_response(
                    stream,
                    StatusCode::Unauthorized,
                    "application/json",
                    response.as_bytes(),
                )
                .await?;
            }
        };
        Ok(())
    }

    pub async fn create_account(
        &self,
        request: &Request,
        stream: &mut TcpStream,
    ) -> Result<(), Box<dyn Error>> {
        let req_data = match request.parse_json::<CreateAccountRequest>(stream).await {
            Some(data) => data,
            None => return Ok(()),
        };

        match self.service.register_user(req_data).await {
            Ok(registered_user) => {
                let response_bytes = serde_json::to_vec(&registered_user)?;
                send_response(
                    stream,
                    StatusCode::Created,
                    "application/json",
                    &response_bytes,
                )
                .await?;
            }
            Err(_) => {
                let response = r#"{"error": "Failed to process password encryption"}"#;
                send_response(
                    stream,
                    StatusCode::InternalServerError,
                    "application/json",
                    response.as_bytes(),
                )
                .await?;
            }
        }
        Ok(())
    }

    pub async fn get_user(
        &self,
        request: &Request,
        stream: &mut TcpStream,
    ) -> Result<(), Box<dyn Error>> {
        let id = request.path.split('?').nth(1).and_then(|query| {
            query
                .split('&')
                .find(|pair| pair.starts_with("id="))
                .and_then(|pair| pair.split('=').nth(1))
                .map(|val| val.to_string())
        });

        let id = match id {
            Some(val) => val,
            None => {
                let response_json = r#"{"error": "Missing id parameter"}"#;
                send_response(
                    stream,
                    StatusCode::BadRequest,
                    "application/json",
                    response_json.as_bytes(),
                )
                .await?;
                return Ok(());
            }
        };

        if let Some(user) = self.service.get_user(&id).await {
            let response_bytes = serde_json::to_vec(&user)?;
            send_response(stream, StatusCode::Ok, "application/json", &response_bytes).await?;
        } else {
            let response_json = r#"{"error": "User not found"}"#;
            send_response(
                stream,
                StatusCode::NotFound,
                "application/json",
                response_json.as_bytes(),
            )
            .await?;
        }
        Ok(())
    }
}

pub async fn not_found(stream: &mut TcpStream) -> Result<(), Box<dyn Error>> {
    let response_json = r#"{"error": "Not found"}"#;
    send_response(
        stream,
        StatusCode::NotFound,
        "application/json",
        response_json.as_bytes(),
    )
    .await?;
    Ok(())
}
