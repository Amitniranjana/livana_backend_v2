use std::sync::Arc;

use crate::services::chat_db_service::ChatDbService;
use crate::services::chat_service::ChatService;
use crate::services::user_service::UserService;
use sqlx::{Pool, Postgres};

#[allow(dead_code)]
/// AppState is the shared state passed into every Axum handler via `State<AppState>`.
#[derive(Clone)]
pub struct AppState {
    pub user_service: Arc<UserService>,
    pub db: Pool<Postgres>,
    pub jwt_secret: String,
    pub chat_service: Arc<ChatService>,
    pub kyc_service: Arc<crate::services::kyc_service::KycService>,
    /// PostgreSQL-backed chat service (chats/messages tables, distinct from Chime).
    pub chat_db_service: Arc<ChatDbService>,
    /// Google OAuth Client ID — used to validate the `aud` field in id_tokens.
    pub google_client_id: String,
}
