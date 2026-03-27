// src/handlers/moderation.rs
//
// Module 9: Moderation APIs
//   9.1  POST /api/v1/users/{id}/block     — Block a user
//   9.2  POST /api/v1/report               — Report an entity
//   9.3  POST /api/v1/chats/{id}/archive   — Archive a chat

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::{
        moderation::{ReportEntityDto, ReportResponseDto},
        response::ApiResponse,
    },
    utils::{api_error::ApiError, auth_extractor::AuthenticationUser},
};

// ---------------------------------------------------------------------------
// 9.1  POST /api/v1/users/{id}/block — Block a user
// ---------------------------------------------------------------------------

pub async fn block_user(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(blocked_user_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let blocker_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    // Cannot block yourself
    if blocker_id == blocked_user_id {
        return Err(ApiError::BadRequest(
            "You cannot block yourself".to_string(),
        ));
    }

    // Verify the target user exists
    let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE id = $1")
        .bind(blocked_user_id)
        .fetch_optional(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    if exists.is_none() {
        return Err(ApiError::NotFound("User not found".to_string()));
    }

    // Idempotent block — ON CONFLICT DO NOTHING
    sqlx::query(
        r#"
        INSERT INTO blocked_users (id, blocker_id, blocked_id, created_at)
        VALUES ($1, $2, $3, NOW())
        ON CONFLICT (blocker_id, blocked_id) DO NOTHING
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(blocker_id)
    .bind(blocked_user_id)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to block user: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "User blocked successfully".to_string(),
        data: serde_json::json!({}),
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// 9.2  POST /api/v1/report — Report an entity
// ---------------------------------------------------------------------------

pub async fn report_entity(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Json(payload): Json<ReportEntityDto>,
) -> Result<impl IntoResponse, ApiError> {
    let reporter_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    if payload.reason.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "Report reason cannot be empty".to_string(),
        ));
    }

    let report_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO moderation_reports (id, reporter_id, entity_type, entity_id, reason, status, created_at)
        VALUES ($1, $2, $3, $4, $5, 'PENDING_REVIEW', NOW())
        "#,
    )
    .bind(report_id)
    .bind(reporter_id)
    .bind(payload.entity_type.to_string())
    .bind(payload.entity_id)
    .bind(&payload.reason)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to submit report: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Report submitted successfully".to_string(),
        data: ReportResponseDto {
            report_id,
            status: "PENDING_REVIEW".to_string(),
        },
    };

    Ok((StatusCode::CREATED, Json(response)))
}

// ---------------------------------------------------------------------------
// 9.3  POST /api/v1/chats/{id}/archive — Archive a chat
// ---------------------------------------------------------------------------

pub async fn archive_chat(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(chat_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    // Verify the user is a participant of this chat
    let is_participant: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id FROM chats
        WHERE id = $1 AND (user1_id = $2 OR user2_id = $2)
        "#,
    )
    .bind(chat_id)
    .bind(user_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    if is_participant.is_none() {
        return Err(ApiError::Forbidden(
            "You are not a participant of this chat".to_string(),
        ));
    }

    // Idempotent archive — ON CONFLICT DO NOTHING
    sqlx::query(
        r#"
        INSERT INTO archived_chats (id, user_id, chat_id, archived_at)
        VALUES ($1, $2, $3, NOW())
        ON CONFLICT (user_id, chat_id) DO NOTHING
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(chat_id)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to archive chat: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Chat archived successfully".to_string(),
        data: serde_json::json!({}),
    };

    Ok((StatusCode::OK, Json(response)))
}
