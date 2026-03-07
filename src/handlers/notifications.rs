// src/handlers/notifications.rs
//
// Module 6: Notification API
//   6.1  GET   /api/v1/notifications
//   6.2  PATCH /api/v1/notifications/{id}/read

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::{
        notifications::{NotificationDto, NotificationReadResponseDto},
        response::ApiResponse,
    },
    utils::{api_error::ApiError, auth_extractor::AuthenticationUser},
};

// ---------------------------------------------------------------------------
// 6.1  GET /api/v1/notifications — Fetch all notifications for the user
// ---------------------------------------------------------------------------

pub async fn get_notifications(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    let rows: Vec<(
        Uuid,
        String,
        String,
        String,
        bool,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        r#"
        SELECT id, title, message, type, is_read, created_at
        FROM notifications
        WHERE user_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let notifications: Vec<NotificationDto> = rows
        .into_iter()
        .map(
            |(id, title, message, notification_type, is_read, created_at)| NotificationDto {
                id,
                title,
                message,
                notification_type,
                is_read,
                created_at,
            },
        )
        .collect();

    let response = ApiResponse {
        success: true,
        message: "Notifications fetched successfully".to_string(),
        data: notifications,
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// 6.2  PATCH /api/v1/notifications/{id}/read — Mark a notification as read
// ---------------------------------------------------------------------------

pub async fn mark_notification_read(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(notification_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    // Verify that the notification exists AND belongs to the authenticated user
    let owner: Option<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM notifications WHERE id = $1"
    )
    .bind(notification_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    match owner {
        None => return Err(ApiError::NotFound("Notification not found".to_string())),
        Some(owner_id) if owner_id != user_id => {
            return Err(ApiError::Forbidden(
                "You do not have permission to modify this notification".to_string(),
            ));
        }
        _ => {}
    }

    // Mark as read
    sqlx::query("UPDATE notifications SET is_read = true WHERE id = $1")
        .bind(notification_id)
        .execute(&app_state.db)
        .await
        .map_err(|e| {
            ApiError::InternalServerError(format!("Failed to mark notification as read: {}", e))
        })?;

    let response = ApiResponse {
        success: true,
        message: "Notification marked as read".to_string(),
        data: NotificationReadResponseDto {
            notification_id,
            is_read: true,
        },
    };

    Ok((StatusCode::OK, Json(response)))
}
