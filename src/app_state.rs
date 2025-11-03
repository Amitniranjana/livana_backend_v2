use std::sync::Arc;
use crate::services::user_service::UserService;

// AppState object will be the shared state and would be passed in controllers
#[derive(Clone, Debug)]
pub struct AppState {
    pub user_service:Arc<UserService>,
}