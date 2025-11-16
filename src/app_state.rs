use std::sync::Arc;
use sqlx::PgPool;
use crate::services::user_service::UserService;
#[allow(dead_code)]
// AppState object will be the shared state and would be passed in controllers
#[derive(Clone, Debug)]
pub struct AppState {
    pub user_service:Arc<UserService>,
      pub db: PgPool,
    pub jwt_secret: String,
}