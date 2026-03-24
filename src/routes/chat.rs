use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::utils::auth_extractor::AuthenticationUser;
use crate::models::chat::{
    ChatRow, ChatItem, ChatListResponse,
    ParticipantInfo, LastMessage, ErrorResponse,
};

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

        -- Sirf current user ke chats
        WHERE cp.user_id = $1

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

pub fn api_chats_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/chats", axum::routing::get(get_chats_handler))
}
