use crate::Extensions;
use crate::Request;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[test]
fn test_extensions() {
    let mut exts = Extensions::new();
    assert!(exts.get::<String>().is_none());

    exts.insert("hello".to_string());
    assert_eq!(exts.get::<String>(), Some(&"hello".to_string()));

    let val = exts.get_mut::<String>().unwrap();
    *val = "world".to_string();
    assert_eq!(exts.get::<String>(), Some(&"world".to_string()));
}

#[tokio::test]
async fn test_request_parse() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let client_task = tokio::spawn(async move {
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(
                b"POST /test?name=rust&age=10 HTTP/1.1\r\n\
            Host: localhost\r\n\
            Content-Length: 17\r\n\
            Content-Type: application/json\r\n\
            \r\n\
            {\"hello\":\"world\"}",
            )
            .await
            .unwrap();
    });

    let (mut server_stream, _) = listener.accept().await.unwrap();
    let request = Request::parse(&mut server_stream).await.unwrap();

    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/test");
    assert_eq!(request.query_params.get("name"), Some(&"rust".to_string()));
    assert_eq!(request.query_params.get("age"), Some(&"10".to_string()));
    assert_eq!(request.headers.get("host"), Some(&"localhost".to_string()));
    assert_eq!(
        request.headers.get("content-type"),
        Some(&"application/json".to_string())
    );
    assert_eq!(
        request.headers.get("content-length"),
        Some(&"17".to_string())
    );
    assert_eq!(request.body, b"{\"hello\":\"world\"}");

    client_task.await.unwrap();
}

#[tokio::test]
async fn test_request_parse_json() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let client_task = tokio::spawn(async move {
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(
                b"POST /test HTTP/1.1\r\n\
            Content-Length: 17\r\n\
            \r\n\
            {\"hello\":\"world\"}",
            )
            .await
            .unwrap();
    });

    let (mut server_stream, _) = listener.accept().await.unwrap();
    let request = Request::parse(&mut server_stream).await.unwrap();

    #[derive(serde::Deserialize, PartialEq, Debug)]
    struct TestBody {
        hello: String,
    }

    let body: TestBody = request.parse_json(&mut server_stream).await.unwrap();
    assert_eq!(
        body,
        TestBody {
            hello: "world".to_string()
        }
    );

    client_task.await.unwrap();
}

#[tokio::test]
async fn test_request_parse_json_invalid() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let client_task = tokio::spawn(async move {
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(
                b"POST /test HTTP/1.1\r\n\
            Content-Length: 5\r\n\
            \r\n\
            hello",
            )
            .await
            .unwrap();

        // Read response from server
        let mut buf = vec![0; 1024];
        let n = stream.read(&mut buf).await.unwrap();
        let response = String::from_utf8_lossy(&buf[..n]);
        assert!(response.contains("HTTP/1.1 400 Bad Request"));
        assert!(response.contains("Invalid JSON payload"));
    });

    let (mut server_stream, _) = listener.accept().await.unwrap();
    let request = Request::parse(&mut server_stream).await.unwrap();

    #[derive(serde::Deserialize)]
    struct TestBody {
        #[allow(dead_code)]
        hello: String,
    }

    let result = request.parse_json::<TestBody>(&mut server_stream).await;
    assert!(result.is_none());

    client_task.await.unwrap();
}
