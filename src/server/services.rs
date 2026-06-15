use crate::server::models::{CreateAccountRequest, UserResponse};
use crate::server::repo::UserRepo;

pub struct Service {
    repo: UserRepo,
}

impl Service {
    pub fn new(repo: UserRepo) -> Self {
        Self {
            repo,
        }
    }
    pub async fn register_user(&self, req: CreateAccountRequest) -> Result<UserResponse, Box<dyn std::error::Error + Send + Sync>> {
        self.repo.insert_user(req.username, req.email, req.password).await
    }
    pub async fn login(&self, email: String, password: String) -> Option<UserResponse> {
        self.repo.login(email, password).await.map(|user| UserResponse {
            id: user.id,
            username: user.username,
            email: user.email,
        })
    }
    pub async fn get_user(&self, id: &uuid::Uuid) -> Option<UserResponse> {
        self.repo.get_user_by_id(id).await.map(|user| UserResponse {
            id: user.id,
            username: user.username,
            email: user.email,
        })
    }
}
