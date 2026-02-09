use std::sync::Arc;

use sqlx::{Pool, Postgres};
use crate::services::user_service::UserService;
use crate::services::chat_service::ChatService; // Added this line
#[allow(dead_code)]
// AppState object will be the shared state and would be passed in controllers
#[derive(Clone)]
pub struct AppState {
    pub user_service: Arc<UserService>,
    pub db: Pool<Postgres>, // Changed from PgPool
    pub jwt_secret: String,
    pub chat_service: Arc<ChatService>,
    pub kyc_service: Arc<crate::services::kyc_service::KycService>,
}