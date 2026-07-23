use axum::{
    routing::{get, post},
    Router,
};

use crate::{
    app_state::AppState,
    handlers::admin_chat::{get_admin_messages, get_admin_threads, send_admin_message},
};

pub fn admin_chat_routes() -> Router<AppState> {
    Router::new()
        .route("/api/chat/admin/messages", post(send_admin_message))
        .route("/api/chat/admin/threads", get(get_admin_threads))
        .route("/api/chat/admin/threads/{thread_id}/messages", get(get_admin_messages))
}
