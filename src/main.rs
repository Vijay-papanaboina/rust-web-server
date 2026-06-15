use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::env;

use tokio::net::{TcpListener, TcpStream};

mod server;

use server::request::handle_request;
use server::routes::Controller;
use server::services::Service;
use server::repo::UserRepo;
use server::models::UserRecord;
use server::middleware::Middleware;
use server::jwt::Jwt;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").await.unwrap();
    println!("Listening on http://127.0.0.1:7878");

    let jwt_secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let jwt = Jwt::new(jwt_secret);
    let middleware = Middleware::new(jwt);
    let users_db: Arc<Mutex<HashMap<String, UserRecord>>> = Arc::new(Mutex::new(HashMap::new()));
    let repo = UserRepo::new(users_db);
    let service = Service::new(repo);
    let controller = Arc::new(Controller::new(service, middleware));

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                eprintln!("Failed to accept connection: {}", e);
                continue;
            }
        };

        let controller = controller.clone();
        tokio::spawn(async move {
            handle_connection(&controller,stream).await;
        });
    }
}

async fn handle_connection(controller: &Controller, mut stream: TcpStream) {
    if let Err(e) = handle_request(controller, &mut stream).await {
        eprintln!("Error handling connection: {}", e);
    }
}
