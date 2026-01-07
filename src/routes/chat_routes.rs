use axum::{Router, routing::{get, post}};
use crate::app_state::AppState;
use crate::handlers::chat_handler::{create_user, create_channel, add_member, send_message, get_auth_creds};

pub fn chat_routes() -> Router<AppState> {
    Router::new()
        .route("/chat/users", post(create_user))
        .route("/chat/channels", post(create_channel))

        .route("/chat/channels/{channel_arn}/members", post(add_member))
        .route("/chat/channels/{channel_arn}/messages", post(send_message))
        .route("/chat/auth", get(get_auth_creds))
}