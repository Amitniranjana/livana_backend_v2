use crate::repository::user_repository::UserRepository;
use crate::models::user::User;
use uuid::Uuid;
use chrono::Utc;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct UserService{
    pub user_repository:UserRepository,
    // in-memory store for reset codes: user_id -> code
    pub reset_codes: Arc<Mutex<HashMap<String, String>>>,
}


impl UserService{
    pub fn new(user_repository: UserRepository)->Self{
        UserService{
            user_repository,
            reset_codes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn create_user(&self, first_name: &str, last_name: &str, email: &str, phone_no: &str, password: &str, _gender: &str) -> Result<User, String> {
        let user = User {
            id: Uuid::new_v4().to_string(),
            first_name: first_name.to_string(),
            phone_no: phone_no.to_string(),
            last_name: last_name.to_string(),
            email: email.to_string(),
            password: password.to_string(),
            user_role: "user".to_string(),
            verified: false,
            last_active: Utc::now().to_string(),
            status: "active".to_string(),
            created_at: Utc::now().to_string(),
            updated_at: Utc::now().to_string(),
        };

        match self.user_repository.create(user.clone()).await {
            Ok(_) => Ok(user),
            Err(e) => Err(e),
        }
    }

    pub async fn find_by_email(&self, email: &str) -> Result<Option<User>, String> {
        self.user_repository.find_by_email(email).await
    }

    pub async fn store_reset_code(&self, user_id: &str, code: &str) -> Result<(), String> {
        let mut map = self.reset_codes.lock().map_err(|e| e.to_string())?;
        map.insert(user_id.to_string(), code.to_string());
        Ok(())
    }

    pub async fn verify_reset_code(&self, code: &str) -> Result<Option<String>, String> {
        let map = self.reset_codes.lock().map_err(|e| e.to_string())?;
        for (uid, c) in map.iter() {
            if c == code {
                return Ok(Some(uid.clone()));
            }
        }
        Ok(None)
    }

    pub async fn update_password(&self, _user_id: &str, _new_password: &str) -> Result<(), String> {
        // TODO: Implement DB update. For now, return Ok to satisfy handlers.
        Ok(())
    }

    pub async fn delete_reset_code(&self, code: &str) -> Result<(), String> {
        let mut map = self.reset_codes.lock().map_err(|e| e.to_string())?;
        let key = map.iter().find_map(|(k, v)| if v == code { Some(k.clone()) } else { None });
        if let Some(k) = key {
            map.remove(&k);
        }
        Ok(())
    }
}