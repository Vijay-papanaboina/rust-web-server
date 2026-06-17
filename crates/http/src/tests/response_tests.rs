use crate::response::{Headers, Response, StatusCode};
use tokio::io::AsyncReadExt;

#[test]
fn test_status_code() {
    assert_eq!(StatusCode::OK.as_u16(), 200);
    assert_eq!(StatusCode::OK.canonical_reason(), Some("OK"));
    assert_eq!(StatusCode::OK.as_str(), "200 OK");

    assert_eq!(StatusCode(999).canonical_reason(), None);
    assert_eq!(StatusCode(999).as_str(), "999");
}

#[test]
fn test_headers() {
    let mut headers = Headers::new();
    headers.set("Content-Type", "application/json");
    headers.set("  X-Custom-Header  ", "  my-value  ");

    assert_eq!(
        headers.get("content-type"),
        Some(&"application/json".to_string())
    );
    assert_eq!(
        headers.get("Content-TYPE"),
        Some(&"application/json".to_string())
    );
    assert_eq!(
        headers.get("x-custom-header"),
        Some(&"my-value".to_string())
    );
}

#[tokio::test]
async fn test_response_send() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let client_task = tokio::spawn(async move {
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        let mut buf = vec![0; 4096];
        let mut n = 0;
        loop {
            let bytes_read = stream.read(&mut buf[n..]).await.unwrap();
            if bytes_read == 0 {
                break;
            }
            n += bytes_read;
            if String::from_utf8_lossy(&buf[..n]).contains("world") {
                break;
            }
        }
        let response_str = String::from_utf8_lossy(&buf[..n]);
        assert!(response_str.contains("HTTP/1.1 201 Created"));
        assert!(response_str.contains("Server: RustWebServer/0.1"));
        assert!(response_str.contains("X-Test: value"));
        assert!(response_str.contains("Content-Type: text/plain"));
        assert!(response_str.contains("Content-Length: 5"));
        assert!(response_str.ends_with("world"));
    });

    let (server_stream, _) = listener.accept().await.unwrap();
    let mut response = Response::new(server_stream);
    response.status(StatusCode::CREATED);
    response.headers.set("x-test", "value");
    response.headers.set("cOnTeNt-TyPe", "text/plain");
    response.send(b"world").await.unwrap();

    client_task.await.unwrap();
}
