use crate::app_state::AppState;
use crate::handlers::chat_handler::{
    add_member, create_channel, create_user, get_auth_creds, get_chat_messages,
    get_chat_messages_wildcard, send_message, upload_chat_media,
};
use axum::{
    Router,
    routing::{get, post},
};

pub fn chat_routes() -> Router<AppState> {
    Router::new()
        .route("/chat/users", post(create_user))
        .route("/chat/channels", post(create_channel))
        .route("/chat/channels/{channel_arn}/members", post(add_member))
        .route("/chat/messages", post(send_message))
        .route("/chat/auth", get(get_auth_creds))
        // ── Media upload & message history ──
        .route("/api/v1/chats/upload", post(upload_chat_media))
        .route("/api/v1/chats/{chat_id}/messages", get(get_chat_messages))
        .route("/api/v1/chats/{*chat_path}", get(get_chat_messages_wildcard))
}
