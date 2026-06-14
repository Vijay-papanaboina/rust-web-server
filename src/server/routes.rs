use crate::server::models::{CreateAccountRequest, LoginRequest, UserResponse};
use crate::server::request::Request;
use crate::server::response::{StatusCode, send_response};
use std::error::Error;
use tokio::net::TcpStream;

pub async fn route(request: &Request, stream: &mut TcpStream) -> Result<(), Box<dyn Error>> {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => index(request, stream).await,
        ("POST", "/login") => login(request, stream).await,
        ("POST", "/register") => create_account(request, stream).await,
        ("GET", "/user") => get_user(request, stream).await,
        _ => not_found(stream).await,
    }
}

async fn index(request: &Request, stream: &mut TcpStream) -> Result<(), Box<dyn Error>> {
    let body = format!(
        "This is a web server written in Rust without any framework.\n\
         You are on the {} {} path.\n",
        request.method, request.path
    );
    send_response(stream, StatusCode::Ok, "text/plain", body.as_bytes()).await?;
    Ok(())
}

async fn login(_request: &Request, stream: &mut TcpStream) -> Result<(), Box<dyn Error>> {
    let response = r#"{"message": "Login successful", "token": "mock-token-xyz"}"#;
    send_response(
        stream,
        StatusCode::Ok,
        "application/json",
        response.as_bytes(),
    )
    .await?;
    Ok(())
}

async fn create_account(_request: &Request, stream: &mut TcpStream) -> Result<(), Box<dyn Error>> {
    let response = r#"{"message": "User registered successfully", "id": 1}"#;
    send_response(
        stream,
        StatusCode::Created,
        "application/json",
        response.as_bytes(),
    )
    .await?;
    Ok(())
}

async fn get_user(_request: &Request, stream: &mut TcpStream) -> Result<(), Box<dyn Error>> {
    let response = r#"{"id": 1, "username": "rust_learner", "email": "rust@example.com"}"#;
    send_response(
        stream,
        StatusCode::Ok,
        "application/json",
        response.as_bytes(),
    )
    .await?;
    Ok(())
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
