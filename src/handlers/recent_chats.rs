// src/handlers/recent_chats.rs
//
// GET /api/v1/chats/recent
//
// Protected by the AuthenticationUser extractor (JWT middleware).
// Calls ChatDbService to fetch the user's recent chats sorted by
// latest message time descending.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde_json::json;

use crate::app_state::AppState;
use crate::utils::auth_extractor::AuthenticationUser;

/// GET /api/v1/chats/recent
///
/// Requires: `Authorization: Bearer <jwt>` header.
/// Returns a JSON array of `{ chat_id, last_message, last_message_time }`.
pub async fn get_recent_chats(
    auth: AuthenticationUser, // JWT validation happens here automatically
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.chat_db_service.get_recent_chats(&auth.user_id).await {
        Ok(chats) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Recent chats fetched successfully",
                "data": chats
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "message": format!("Failed to fetch recent chats: {}", e),
                "data": null
            })),
        ),
    }
}
