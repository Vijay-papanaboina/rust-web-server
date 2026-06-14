use tokio::net::{TcpListener, TcpStream};

mod server;

use server::request::handle_request;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").await.unwrap();
    println!("Listening on http://127.0.0.1:7878");

    loop {
        let (stream, _) = listener.accept().await.unwrap();

        tokio::spawn(async move {
            handle_connection(stream).await;
        });
    }
}

async fn handle_connection(mut stream: TcpStream) {
    if let Err(e) = handle_request(&mut stream).await {
        eprintln!("Error handling connection: {}", e);
    }
}
