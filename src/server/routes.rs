use std::error::Error;
use std::sync::Arc;
use tokio::net::TcpStream;

use crate::server::jwt::Claims;
use crate::server::middleware::Middleware;
use crate::server::models::{CreateAccountRequest, LoginRequest, LoginResponse};
use crate::server::services::Service;
use http::Request;
use http::response::{Response, StatusCode};
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn route(
    controller: &Controller,
    request: &mut Request,
    stream: TcpStream,
) -> Result<(), Box<dyn Error>> {
    let mut response = Response::new(stream);
    // Check request path ignoring query parameters for matching routes
    let path = request.path.split('?').next().unwrap_or("/");

    match (request.method.as_str(), path) {
        ("GET", "/") => controller.index(request, &mut response).await,
        ("POST", "/login") => controller.login(request, &mut response).await,
        ("POST", "/register") => controller.create_account(request, &mut response).await,
        ("GET", "/user") => {
            controller
                .middleware
                .check_auth(request, &mut response)
                .await?;
            controller.get_user(request, &mut response).await
        }
        ("GET", "/ws") => controller.ws_upgrade(request, response).await,
        _ => not_found(&mut response).await,
    }
}

pub struct Controller {
    pub service: Service,
    pub middleware: Middleware,
    pub chat_manager: Arc<crate::server::chat::ChatManager>,
}

impl Controller {
    pub fn new(
        service: Service,
        middleware: Middleware,
        chat_manager: Arc<crate::server::chat::ChatManager>,
    ) -> Self {
        Self {
            service,
            middleware,
            chat_manager,
        }
    }

    pub async fn index(
        &self,
        request: &Request,
        response: &mut Response,
    ) -> Result<(), Box<dyn Error>> {
        let body = format!(
            "This is a web server written in Rust without any framework.\n\
             You are on the {} {} path.\n",
            request.method, request.path
        );
        response.status(StatusCode::OK);
        response.headers.set("Content-Type", "text/plain");
        response.send(body.as_bytes()).await?;
        Ok(())
    }

    pub async fn login(
        &self,
        request: &Request,
        response: &mut Response,
    ) -> Result<(), Box<dyn Error>> {
        let req_data = match request
            .parse_json::<LoginRequest>(response.stream_mut())
            .await
        {
            Some(data) => data,
            None => return Ok(()),
        };

        let db_response = self.service.login(req_data.email, req_data.password).await;
        match db_response {
            Some(user) => {
                let exp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as usize
                    + 24 * 3600; // 24 hours
                let claims = Claims {
                    sub: user.id.to_string(),
                    exp,
                };
                let token = match self
                    .middleware
                    .jwt
                    .encode(&claims)
                    .map_err(|e| e.to_string())
                {
                    Ok(t) => t,
                    Err(err_msg) => {
                        eprintln!("JWT encode error: {}", err_msg);
                        let response_body = r#"{"error": "Failed to generate token"}"#;
                        response.status(StatusCode::INTERNAL_SERVER_ERROR);
                        response.headers.set("Content-Type", "application/json");
                        response.send(response_body.as_bytes()).await?;
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
                response.status(StatusCode::OK);
                response.headers.set("Content-Type", "application/json");
                response.send(&response_bytes).await?;
            }
            None => {
                let response_body = r#"{"error": "Invalid credentials"}"#;
                response.status(StatusCode::UNAUTHORIZED);
                response.headers.set("Content-Type", "application/json");
                response.send(response_body.as_bytes()).await?;
            }
        };
        Ok(())
    }

    pub async fn create_account(
        &self,
        request: &Request,
        response: &mut Response,
    ) -> Result<(), Box<dyn Error>> {
        let req_data = match request
            .parse_json::<CreateAccountRequest>(response.stream_mut())
            .await
        {
            Some(data) => data,
            None => return Ok(()),
        };

        match self.service.register_user(req_data).await {
            Ok(registered_user) => {
                let response_bytes = serde_json::to_vec(&registered_user)?;
                response.status(StatusCode::CREATED);
                response.headers.set("Content-Type", "application/json");
                response.send(&response_bytes).await?;
            }
            Err(_) => {
                let response_body = r#"{"error": "Failed to process password encryption"}"#;
                response.status(StatusCode::INTERNAL_SERVER_ERROR);
                response.headers.set("Content-Type", "application/json");
                response.send(response_body.as_bytes()).await?;
            }
        }
        Ok(())
    }

    pub async fn get_user(
        &self,
        request: &Request,
        response: &mut Response,
    ) -> Result<(), Box<dyn Error>> {
        let user_id = match request.extensions.get::<Claims>() {
            Some(claims) => &claims.sub,
            None => {
                let response_json = r#"{"error": "Unauthorized"}"#;
                response.status(StatusCode::UNAUTHORIZED);
                response.headers.set("Content-Type", "application/json");
                response.send(response_json.as_bytes()).await?;
                return Ok(());
            }
        };

        let parsed_uuid = match uuid::Uuid::parse_str(user_id) {
            Ok(u) => u,
            Err(_) => {
                let response_json = r#"{"error": "Invalid user ID in token"}"#;
                response.status(StatusCode::BAD_REQUEST);
                response.headers.set("Content-Type", "application/json");
                response.send(response_json.as_bytes()).await?;
                return Ok(());
            }
        };

        if let Some(user) = self.service.get_user(&parsed_uuid).await {
            let response_bytes = serde_json::to_vec(&user)?;
            response.status(StatusCode::OK);
            response.headers.set("Content-Type", "application/json");
            response.send(&response_bytes).await?;
        } else {
            let response_json = r#"{"error": "User not found"}"#;
            response.status(StatusCode::NOT_FOUND);
            response.headers.set("Content-Type", "application/json");
            response.send(response_json.as_bytes()).await?;
        }
        Ok(())
    }

    pub async fn ws_upgrade(
        &self,
        request: &Request,
        response: Response,
    ) -> Result<(), Box<dyn Error>> {
        let stream = response.into_stream();
        match crate::server::websocket::upgrade(request, stream).await {
            Ok(receiver) => {
                self.chat_manager.handle_connection(receiver);
            }
            Err(e) => {
                eprintln!("WebSocket upgrade failed: {}", e);
            }
        }
        Ok(())
    }
}

pub async fn not_found(response: &mut Response) -> Result<(), Box<dyn Error>> {
    let response_json = r#"{"error": "Not found"}"#;
    response.status(StatusCode::NOT_FOUND);
    response.headers.set("Content-Type", "application/json");
    response.send(response_json.as_bytes()).await?;
    Ok(())
}
