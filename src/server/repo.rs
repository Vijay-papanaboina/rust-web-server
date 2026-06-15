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
    
    pub async fn insert_user(&self, username: String, email: String, password: String) -> UserResponse {
        let id = uuid::Uuid::new_v4().to_string();
    
        let record = UserRecord {
            id: id.clone(),
            username: username.clone(),
            email: email.clone(),
            password,
        };
    
        self.users.lock().unwrap().insert(email.clone(), record);
    
        UserResponse {
            id,
            username,
            email,
        }
    }
    
    pub async fn login(&self, email: String, password: String) -> Option<UserRecord> {
        let user_login = self.users.lock().unwrap().get(&email).cloned();
        match user_login {
            Some(user) => {
                if user.password == password {
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