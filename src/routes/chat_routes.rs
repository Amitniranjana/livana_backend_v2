use crate::app_state::AppState;
use crate::handlers::chat_handler::{
    add_member, create_channel, create_user, get_auth_creds, send_message,
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
}
