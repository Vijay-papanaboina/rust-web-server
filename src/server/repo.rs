use crate::server::models::{UserRecord, UserResponse};
use sqlx::PgPool;

pub struct UserRepo {
    pool: PgPool,
}

impl UserRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl UserRepo {
    pub async fn insert_user(
        &self,
        username: String,
        email: String,
        password: String,
    ) -> Result<UserResponse, Box<dyn std::error::Error + Send + Sync>> {
        let id = uuid::Uuid::new_v4();
        let hashed_password = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;

        let _res = sqlx::query(
            "INSERT INTO users (id, username, email, password)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(&id)
        .bind(&username)
        .bind(&email)
        .bind(&hashed_password)
        .execute(&self.pool)
        .await?;

        Ok(UserResponse {
            id,
            username,
            email,
        })
    }

    pub async fn login(&self, email: String, password: String) -> Option<UserRecord> {
        let user_login = sqlx::query_as::<_, UserRecord>(
            "SELECT * FROM users
             WHERE email = $1",
        )
        .bind(&email)
        .fetch_optional(&self.pool)
        .await
        .unwrap_or(None);
        match user_login {
            Some(user) => {
                if bcrypt::verify(password, &user.password).unwrap_or(false) {
                    Some(user)
                } else {
                    None
                }
            }
            None => None,
        }
    }

    pub async fn get_user_by_id(&self, id: &uuid::Uuid) -> Option<UserRecord> {
        match sqlx::query_as::<_, UserRecord>(
            "SELECT * FROM users
             WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        {
            Ok(user) => user,
            Err(e) => {
                eprintln!("Database error in get_user_by_id: {:?}", e);
                None
            }
        }
    }
}
