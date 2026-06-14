use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum StatusCode {
    Ok,
    Created,
    BadRequest,
    Unauthorized,
    NotFound,
    InternalServerError,
}

impl StatusCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            StatusCode::Ok => "200 OK",
            StatusCode::Created => "201 Created",
            StatusCode::BadRequest => "400 Bad Request",
            StatusCode::Unauthorized => "401 Unauthorized",
            StatusCode::NotFound => "404 Not Found",
            StatusCode::InternalServerError => "500 Internal Server Error",
        }
    }
}

pub async fn send_response(stream: &mut TcpStream, status: StatusCode, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         \r\n",
        status.as_str(),
        content_type,
        body.len()
    );
    let _ = stream.write_all(header.as_bytes()).await;
    let _ = stream.write_all(body).await;
}
