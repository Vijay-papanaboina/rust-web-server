use crate::mrequest::Request;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

async fn send_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         \r\n",
        status,
        content_type,
        body.len()
    );
    let _ = stream.write_all(header.as_bytes()).await;
    let _ = stream.write_all(body).await;
}

pub async fn test_path(stream: &mut TcpStream, request: &Request) {
    let response_json = serde_json::to_string_pretty(&request).unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    send_response(stream, "200 OK", "text/plain", response_json.as_bytes()).await;
}

pub async fn index(request: &Request, stream: &mut TcpStream) {
    let body = format!(
        "This is a web server written in Rust without any framework.\n\
    You are on the {} {} path.\n",
        request.method, request.path
    );
    send_response(stream, "200 OK", "text/plain", body.as_bytes()).await;
}

pub async fn echo(request: &Request, stream: &mut TcpStream) {
    test_path(stream, request).await;
}

#[allow(dead_code)]
pub async fn json(request: &Request, stream: &mut TcpStream) {
    test_path(stream, request).await;
}

pub async fn not_found(stream: &mut TcpStream) {
    let response_json = r#"{"error": "Not found"}"#;
    let status_code = "404 Not Found";
    send_response(
        stream,
        status_code,
        "application/json",
        response_json.as_bytes(),
    ).await;
}
