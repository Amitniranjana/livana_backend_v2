use crate::app_state::AppState;
use crate::handlers::chat_handler::{
    add_member, create_channel, create_user, get_auth_creds, get_chat_messages,
    get_chat_messages_arn_path, get_chat_messages_by_channel, send_message, upload_chat_media,
    mark_chat_seen,
};
use crate::handlers::ws_handler::ws_handler;
use axum::{
    Router,
    routing::{get, post, patch},
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
        .route("/api/v1/chats/{chat_id}/seen", patch(mark_chat_seen))
        // ── WebSocket for Live Notifications ──
        .route("/api/v1/ws", get(ws_handler))
        // ── Chime ARN-based message fetch (Flutter sends full ARN) ──
        .route(
            "/api/v1/chats/channel/messages",
            get(get_chat_messages_by_channel),
        )
        // ── Full ARN inside the Path (4 segments) ──
        .route(
            "/api/v1/chats/{arn_prefix}/{app_id}/channel/{channel_id}/messages",
            get(get_chat_messages_arn_path),
        )
}
