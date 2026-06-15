use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::server::models::{UserResponse, UserRecord};

pub struct UserRepo {
    users: Arc<Mutex<HashMap<String, UserRecord>>>,
}

impl UserRepo {
    pub fn new(users: Arc<Mutex<HashMap<String, UserRecord>>>) -> Self {
        Self {
            users,
        }
    }
}

impl UserRepo {
    
    pub async fn insert_user(&self, username: String, email: String, password: String) -> Result<UserResponse, bcrypt::BcryptError> {
        let id = uuid::Uuid::new_v4().to_string();
        let hashed_password = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;
    
        let record = UserRecord {
            id: id.clone(),
            username: username.clone(),
            email: email.clone(),
            password: hashed_password,
        };
    
        self.users.lock().unwrap().insert(email.clone(), record);
    
        Ok(UserResponse {
            id,
            username,
            email,
        })
    }
    
    pub async fn login(&self, email: String, password: String) -> Option<UserRecord> {
        let user_login = self.users.lock().unwrap().get(&email).cloned();
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

    pub async fn get_user_by_id(&self, id: &str) -> Option<UserRecord> {
        let users = self.users.lock().unwrap();
        users.values().find(|u| u.id == id).cloned()
    }
}