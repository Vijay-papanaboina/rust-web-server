use sqlx::PgPool;
use std::env;
use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};

mod server;

use server::jwt::Jwt;
use server::middleware::Middleware;
use server::repo::UserRepo;
use server::request::Request;
use server::routes::{self, Controller};
use server::services::Service;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").await.unwrap();
    println!("Listening on http://127.0.0.1:7878");

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to PostgreSQL database");

    // Automatically initialize database schema if not present
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
            username VARCHAR(255) NOT NULL,
            email VARCHAR(255) UNIQUE NOT NULL,
            password VARCHAR(255) NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );",
    )
    .execute(&pool)
    .await
    .expect("Failed to initialize database schema");

    let jwt_secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let jwt = Jwt::new(jwt_secret);
    let middleware = Middleware::new(jwt);
    let repo = UserRepo::new(pool);
    let service = Service::new(repo);
    let chat_manager = Arc::new(server::chat::ChatManager::new());
    let controller = Arc::new(Controller::new(service, middleware, chat_manager));

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
            handle_connection(&controller, stream).await;
        });
    }
}

async fn handle_connection(controller: &Controller, mut stream: TcpStream) {
    match Request::parse(&mut stream).await {
        Ok(mut request) => {
            server::middleware::logger(&request);
            if let Err(e) = routes::route(controller, &mut request, stream).await {
                eprintln!("Error routing request: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Failed to parse request: {}", e);
            let _ = routes::not_found(&mut stream).await;
        }
    }
}
