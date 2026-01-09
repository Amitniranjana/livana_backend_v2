use axum::{
    extract::{State, Path, Json},
    http::StatusCode,
    response::IntoResponse,
};
use crate::app_state::AppState;
use crate::models::chat::{CreateUserRequest, CreateChannelRequest, AddMemberRequest, SendMessageRequest};
use serde_json::json;

/// Create User
pub async fn create_user(
    State(app_state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> impl IntoResponse {
    // NOTE: In a real app, `app_instance_arn` should probably be in config/env
    let app_instance_arn = std::env::var("CHIME_APP_INSTANCE_ARN").unwrap_or_default();

    match app_state.chat_service.create_app_instance_user(&app_instance_arn, &payload.user_id, &payload.username).await {
        Ok(user) => (StatusCode::CREATED, Json(user)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// Create Channel
pub async fn create_channel(
    State(app_state): State<AppState>,
    Json(payload): Json<CreateChannelRequest>,
) -> impl IntoResponse {
    let app_instance_arn = std::env::var("CHIME_APP_INSTANCE_ARN").unwrap_or_default();
    // For demo, we assume the creator is passed in user_arns[0] or handled via auth middleware context.
    // Here using a simpler approach: the first user in list is the creator
    let cleaner_arn = payload.user_arns.first().cloned().unwrap_or_default();

    match app_state.chat_service.create_channel(&app_instance_arn, &payload.name, &payload.mode, &payload.privacy, &cleaner_arn).await {
        Ok(channel) => (StatusCode::CREATED, Json(channel)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// Add Member
pub async fn add_member(
    State(app_state): State<AppState>,
    Path(channel_arn): Path<String>,
    Json(payload): Json<AddMemberRequest>,
) -> impl IntoResponse {
    // Helper to extract whoever is making the request (admin)
    let admin_arn = std::env::var("CHIME_ADMIN_ARN").unwrap_or_default(); // Placeholder

    match app_state.chat_service.add_channel_flow(&channel_arn, &payload.member_arn, &admin_arn).await {
        Ok(_) => (StatusCode::OK, Json(json!({"message": "Member added"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// Send Message
pub async fn send_message(
    State(app_state): State<AppState>,
    Path(channel_arn): Path<String>,
    Json(payload): Json<SendMessageRequest>,
) -> impl IntoResponse {
    // In real app, extracting sender from Auth token
    let sender_arn = std::env::var("CHIME_ADMIN_ARN").unwrap_or_default();

    match app_state.chat_service.send_message(&channel_arn, &payload.content, &sender_arn).await {
        Ok(msg_id) => (StatusCode::OK, Json(json!({"message_id": msg_id}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// Get WebSocket Endpoint
pub async fn get_auth_creds(
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    match app_state.chat_service.get_messaging_endpoint().await {
        Ok(endpoint) => (StatusCode::OK, Json(json!({"endpoint": endpoint}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}
