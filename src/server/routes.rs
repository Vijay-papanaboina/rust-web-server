use crate::server::request::Request;
use crate::server::response::{send_response, StatusCode};
use tokio::net::TcpStream;

pub async fn route(request: &Request, stream: &mut TcpStream) {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => index(request, stream).await,
        ("POST", "/login") => login(request, stream).await,
        ("POST", "/register") => create_account(request, stream).await,
        ("GET", "/user") => get_user(request, stream).await,
        _ => not_found(stream).await,
    }
}

async fn index(request: &Request, stream: &mut TcpStream) {
    let body = format!(
        "This is a web server written in Rust without any framework.\n\
         You are on the {} {} path.\n",
        request.method, request.path
    );
    send_response(stream, StatusCode::Ok, "text/plain", body.as_bytes()).await;
}

async fn login(_request: &Request, stream: &mut TcpStream) {
    let response = r#"{"message": "Login successful", "token": "mock-token-xyz"}"#;
    send_response(stream, StatusCode::Ok, "application/json", response.as_bytes()).await;
}

async fn create_account(_request: &Request, stream: &mut TcpStream) {
    let response = r#"{"message": "User registered successfully", "id": 1}"#;
    send_response(stream, StatusCode::Created, "application/json", response.as_bytes()).await;
}

async fn get_user(_request: &Request, stream: &mut TcpStream) {
    let response = r#"{"id": 1, "username": "rust_learner", "email": "rust@example.com"}"#;
    send_response(stream, StatusCode::Ok, "application/json", response.as_bytes()).await;
}

pub async fn not_found(stream: &mut TcpStream) {
    let response_json = r#"{"error": "Not found"}"#;
    send_response(
        stream,
        StatusCode::NotFound,
        "application/json",
        response_json.as_bytes(),
    ).await;
}
