use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get},
};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::models::chat::{
    ChatExistsRow, ChatItem, ChatListResponse, ChatRow, ErrorResponse, LastMessage, ParticipantInfo,
};
use crate::utils::auth_extractor::AuthenticationUser;

pub async fn get_chats_handler(
    auth: AuthenticationUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // parse user_id
    let user_id = match uuid::Uuid::parse_str(&auth.user_id) {
        Ok(id) => id,
        Err(_) => {
            let error = ErrorResponse {
                success: false,
                message: "Unauthorized. Invalid user ID format in token.".to_string(),
                error_code: "INVALID_TOKEN".to_string(),
            };
            return (StatusCode::UNAUTHORIZED, Json(serde_json::json!(error))).into_response();
        }
    };

    // Database query optimized with LATERAL JOINs
    let query = r#"
        SELECT
            c.id                            AS chat_id,
            u.id                            AS participant_id,
            (u.first_name || ' ' || u.last_name) AS participant_name,
            u.profile_picture               AS participant_image,
            lm.message                      AS last_message_text,
            lm.created_at                   AS last_message_time,
            0                               AS unread_count
        FROM chat_participants cp

        -- Current user ke sab chats
        JOIN chats c
            ON c.id = cp.chat_id

        -- Dusra participant dhundo (current user nahi)
        JOIN chat_participants cp2
            ON cp2.chat_id = c.id
            AND cp2.user_id != $1

        -- Dusre participant ki user info
        JOIN users u
            ON u.id = cp2.user_id

        -- Har chat ka LAST MESSAGE (LATERAL join = efficient)
        LEFT JOIN LATERAL (
            SELECT content AS message, created_at
            FROM messages
            WHERE chat_id = c.id
            ORDER BY created_at DESC
            LIMIT 1
        ) lm ON true

        -- Sirf current user ke chats + deleted chats hide karo
        WHERE cp.user_id = $1
          AND c.is_deleted = FALSE

        -- Latest chat pehle dikhao
        ORDER BY lm.created_at DESC NULLS LAST
    "#;

    let rows = sqlx::query_as::<_, ChatRow>(query)
        .bind(user_id)
        .fetch_all(&state.db)
        .await;

    let rows = match rows {
        Ok(data) => data,
        Err(e) => {
            println!("Database error in get_chats: {:?}", e);
            let error = ErrorResponse {
                success: false,
                message: "Internal server error".to_string(),
                error_code: "DATABASE_ERROR".to_string(),
            };
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!(error)),
            )
                .into_response();
        }
    };

    let chat_items: Vec<ChatItem> = rows
        .into_iter()
        .map(|row| {
            let last_message = match (row.last_message_text, row.last_message_time) {
                (Some(text), Some(time)) => Some(LastMessage {
                    text,
                    timestamp: time,
                }),
                _ => None,
            };

            ChatItem {
                chat_id: row.chat_id,
                participant: ParticipantInfo {
                    id: row.participant_id,
                    name: row.participant_name,
                    profile_image: row.participant_image,
                },
                last_message,
                unread_count: row.unread_count,
            }
        })
        .collect();

    let response = ChatListResponse {
        success: true,
        data: chat_items,
    };

    (StatusCode::OK, Json(serde_json::json!(response))).into_response()
}

// ─────────────────────────────────────────────────────────
// DELETE /api/chats/{chat_id} — Soft Delete
// ─────────────────────────────────────────────────────────
pub async fn delete_chat_handler(
    auth: AuthenticationUser,
    State(state): State<AppState>,
    Path(chat_id): Path<Uuid>,
) -> impl IntoResponse {
    // STEP 1: JWT se user_id lo
    let user_id = match Uuid::parse_str(&auth.user_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "message": "Invalid or missing token",
                    "error_code": "INVALID_TOKEN"
                })),
            )
                .into_response();
        }
    };

    // STEP 2: Chat exist karti hai ya nahi
    let chat_check = sqlx::query_as::<_, ChatExistsRow>(
        "SELECT id AS chat_id, is_deleted FROM chats WHERE id = $1",
    )
    .bind(chat_id)
    .fetch_optional(&state.db)
    .await;

    let chat = match chat_check {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "message": "Chat not found",
                    "error_code": "CHAT_NOT_FOUND"
                })),
            )
                .into_response();
        }
        Err(e) => {
            println!("DB error checking chat existence: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Internal server error",
                    "error_code": "DATABASE_ERROR"
                })),
            )
                .into_response();
        }
    };

    // STEP 3: Already deleted hai to CHAT_NOT_FOUND
    if chat.is_deleted {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "message": "Chat not found",
                "error_code": "CHAT_NOT_FOUND"
            })),
        )
            .into_response();
    }

    // STEP 4: Current user is participant check karo
    let is_participant = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM chat_participants WHERE chat_id = $1 AND user_id = $2)",
    )
    .bind(chat_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await;

    match is_participant {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "success": false,
                    "message": "You are not a participant of this chat",
                    "error_code": "ACCESS_DENIED"
                })),
            )
                .into_response();
        }
        Err(e) => {
            println!("DB error checking participant: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Internal server error",
                    "error_code": "DATABASE_ERROR"
                })),
            )
                .into_response();
        }
    }

    // STEP 5: Soft delete karo
    let now = Utc::now();
    let result = sqlx::query("UPDATE chats SET is_deleted = TRUE, deleted_at = $1 WHERE id = $2")
        .bind(now)
        .bind(chat_id)
        .execute(&state.db)
        .await;

    match result {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Chat deleted successfully"
            })),
        )
            .into_response(),
        Err(e) => {
            println!("DB error soft deleting chat: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Internal server error",
                    "error_code": "DATABASE_ERROR"
                })),
            )
                .into_response()
        }
    }
}

// ─────────────────────────────────────────────────────────
// ROUTES
// ─────────────────────────────────────────────────────────
pub fn api_chats_routes() -> Router<AppState> {
    Router::new()
        .route("/api/chats", get(get_chats_handler))
        .route("/api/chats/{chat_id}", delete(delete_chat_handler))
}
