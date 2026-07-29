use axum::{
    routing::{get, delete},
    Router, middleware,
};
use crate::{
    app_state::AppState,
    handlers::admin_user_chats::{
        get_all_user_chats_admin,
        get_user_chat_messages_admin,
        force_delete_chat_admin,
        force_delete_message_admin,
    },
    utils::admin_auth_guard::admin_auth_guard,
};

pub fn admin_user_chats_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/admin/chats", get(get_all_user_chats_admin))
        .route("/api/admin/chats/{id}/messages", get(get_user_chat_messages_admin))
        .route("/api/admin/chats/{id}/force", delete(force_delete_chat_admin))
        .route("/api/admin/messages/{id}/force", delete(force_delete_message_admin))
        .route_layer(middleware::from_fn_with_state(state, admin_auth_guard))
}
