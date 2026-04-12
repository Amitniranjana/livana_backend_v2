// src/handlers/vibes.rs
//
// Module 10: Vibe API (Shared Apartment Matchmaking)
//   10.1  POST /api/v1/vibes              — Send a vibe
//   10.2  POST /api/v1/vibes/{id}/accept  — Accept a vibe
//   10.3  POST /api/v1/vibes/{id}/reject  — Reject a vibe
//   10.4  GET  /api/v1/vibes/matches      — Get all matches

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
        response::ApiResponse,
        vibes::{MatchDto, SendVibeDto, VibeResponseDto},
    },
    utils::{api_error::ApiError, auth_extractor::AuthenticationUser},
};

// ---------------------------------------------------------------------------
// 10.1  POST /api/v1/vibes — Send a vibe to another user
// ---------------------------------------------------------------------------

pub async fn send_vibe(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Json(payload): Json<SendVibeDto>,
) -> Result<impl IntoResponse, ApiError> {
    let sender_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    // Cannot send a vibe to yourself
    if sender_id == payload.target_user_id {
        return Err(ApiError::BadRequest(
            "You cannot send a vibe to yourself".to_string(),
        ));
    }

    let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE id = $1")
        .bind(payload.target_user_id)
        .fetch_optional(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    if exists.is_none() {
        return Err(ApiError::NotFound("Target user not found".to_string()));
    }

    // Verify the property exists
    let property_exists: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM properties WHERE id = $1")
            .bind(payload.property_id)
            .fetch_optional(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    if property_exists.is_none() {
        return Err(ApiError::NotFound("Property not found".to_string()));
    }

    let vibe_id = Uuid::new_v4();

    // Insert with ON CONFLICT — handle duplicate pending vibes gracefully
    let result = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO vibes (id, sender_id, target_user_id, property_id, status, created_at)
        VALUES ($1, $2, $3, $4, 'pending', NOW())
        ON CONFLICT (sender_id, target_user_id, property_id) DO UPDATE SET id = vibes.id
        RETURNING id
        "#,
    )
    .bind(vibe_id)
    .bind(sender_id)
    .bind(payload.target_user_id)
    .bind(payload.property_id)
    .fetch_one(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to send vibe: {}", e)))?;

    // ── Trigger notification + chat (best-effort, don't fail the API) ──
    {
        use crate::utils::notification_chat_helper::{
            create_chat_if_not_exists, create_notification, get_user_display_name,
        };

        let db = &app_state.db;
        let sender_name = get_user_display_name(db, sender_id)
            .await
            .unwrap_or_else(|_| "A user".to_string());

        // Notify the target user
        if let Err(e) = create_notification(
            db,
            payload.target_user_id,
            "New Vibe! 💫",
            &format!("{} showed interest in your property", sender_name),
            "VIBE",
            Some(result),
            Some("VIBE"),
        )
        .await
        {
            println!("[Vibe] Failed to create notification: {}", e);
        }

        // Auto-create chat if not exists + insert initial message
        if let Err(e) = create_chat_if_not_exists(
            db,
            sender_id,
            payload.target_user_id,
            &format!("👋 {} is interested in your property", sender_name),
        )
        .await
        {
            println!("[Vibe] Failed to create chat: {}", e);
        }
    }

    let response = ApiResponse {
        success: true,
        message: "Vibe sent successfully".to_string(),
        data: VibeResponseDto {
            vibe_id: result,
            sender_id,
            target_user_id: payload.target_user_id,
            property_id: payload.property_id,
            status: "pending".to_string(),
        },
    };

    Ok((StatusCode::CREATED, Json(response)))
}

// ---------------------------------------------------------------------------
// 10.2  POST /api/v1/vibes/{id}/accept — Accept a vibe (creates a match)
// ---------------------------------------------------------------------------

pub async fn accept_vibe(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(vibe_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    // Fetch the vibe and verify ownership
    let vibe: Option<(Uuid, String, Uuid)> =
        sqlx::query_as("SELECT target_user_id, status, sender_id FROM vibes WHERE id = $1")
            .bind(vibe_id)
            .fetch_optional(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let origin_sender_id = match vibe {
        None => return Err(ApiError::NotFound("Vibe not found".to_string())),
        Some((target_id, _, _)) if target_id != user_id => {
            return Err(ApiError::Forbidden(
                "Only the target user can accept this vibe".to_string(),
            ));
        }
        Some((_, ref status, _)) if status != "pending" => {
            return Err(ApiError::BadRequest(format!(
                "Vibe has already been {}",
                status.to_lowercase()
            )));
        }
        Some((_, _, s_id)) => s_id,
    };

    // Update status to ACCEPTED
    sqlx::query("UPDATE vibes SET status = 'accepted', updated_at = NOW() WHERE id = $1")
        .bind(vibe_id)
        .execute(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Failed to accept vibe: {}", e)))?;

    // Update action_status on the related notification (best-effort)
    let _ = sqlx::query(
        r#"
        UPDATE notifications
        SET action_status = 'ACCEPTED'
        WHERE related_entity_id = $1
          AND related_entity_type = 'VIBE'
        "#,
    )
    .bind(vibe_id)
    .execute(&app_state.db)
    .await
    .map_err(|e| println!("[Vibe] Failed to update notification action_status: {}", e));

    use crate::utils::notification_chat_helper::{create_notification, get_user_display_name};
    let target_name = get_user_display_name(&app_state.db, user_id)
        .await
        .unwrap_or_else(|_| "A user".to_string());

    // Notify the original sender
    if let Err(e) = create_notification(
        &app_state.db,
        origin_sender_id,
        "Vibe Accepted! 🎉",
        &format!("{} accepted your vibe!", target_name),
        "VIBE_STATUS",
        Some(vibe_id),
        Some("VIBE"),
    )
    .await
    {
        println!("[Vibe] Failed to notify sender about accepted vibe: {}", e);
    }

    let response = ApiResponse {
        success: true,
        message: "Vibe accepted. It's a match!".to_string(),
        data: serde_json::json!({}),
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// 10.3  POST /api/v1/vibes/{id}/reject — Reject a vibe
// ---------------------------------------------------------------------------

pub async fn reject_vibe(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(vibe_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    // Fetch the vibe and verify ownership
    let vibe: Option<(Uuid, String, Uuid)> =
        sqlx::query_as("SELECT target_user_id, status, sender_id FROM vibes WHERE id = $1")
            .bind(vibe_id)
            .fetch_optional(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let origin_sender_id = match vibe {
        None => return Err(ApiError::NotFound("Vibe not found".to_string())),
        Some((target_id, _, _)) if target_id != user_id => {
            return Err(ApiError::Forbidden(
                "Only the target user can reject this vibe".to_string(),
            ));
        }
        Some((_, ref status, _)) if status != "pending" => {
            return Err(ApiError::BadRequest(format!(
                "Vibe has already been {}",
                status.to_lowercase()
            )));
        }
        Some((_, _, s_id)) => s_id,
    };

    // Update status to REJECTED
    sqlx::query("UPDATE vibes SET status = 'rejected', updated_at = NOW() WHERE id = $1")
        .bind(vibe_id)
        .execute(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Failed to reject vibe: {}", e)))?;

    // Update action_status on the related notification (best-effort)
    let _ = sqlx::query(
        r#"
        UPDATE notifications
        SET action_status = 'REJECTED'
        WHERE related_entity_id = $1
          AND related_entity_type = 'VIBE'
        "#,
    )
    .bind(vibe_id)
    .execute(&app_state.db)
    .await
    .map_err(|e| println!("[Vibe] Failed to update notification action_status: {}", e));

    use crate::utils::notification_chat_helper::{create_notification, get_user_display_name};
    let target_name = get_user_display_name(&app_state.db, user_id)
        .await
        .unwrap_or_else(|_| "A user".to_string());

    // Notify the original sender
    if let Err(e) = create_notification(
        &app_state.db,
        origin_sender_id,
        "Vibe Update",
        &format!("{} passed on your vibe", target_name),
        "VIBE_STATUS",
        Some(vibe_id),
        Some("VIBE"),
    )
    .await
    {
        println!("[Vibe] Failed to notify sender about rejected vibe: {}", e);
    }

    let response = ApiResponse {
        success: true,
        message: "Vibe rejected".to_string(),
        data: serde_json::json!({}),
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// 10.4  GET /api/v1/vibes/matches — Get all accepted matches
// ---------------------------------------------------------------------------

pub async fn get_matches(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    // Find all ACCEPTED vibes where user is sender OR target,
    // and join the OTHER user's profile info.
    let rows: Vec<(
        Uuid,
        Uuid,
        String,
        String,
        Option<String>,
        Uuid,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        r#"
            SELECT
                v.id AS match_id,
                CASE
                    WHEN v.sender_id = $1 THEN v.target_user_id
                    ELSE v.sender_id
                END AS matched_user_id,
                u.first_name,
                u.last_name,
                u.profile_image,
                v.property_id,
                v.updated_at AS matched_at
            FROM vibes v
            JOIN users u ON u.id = CASE
                WHEN v.sender_id = $1 THEN v.target_user_id
                ELSE v.sender_id
            END
            WHERE v.status = 'accepted'
              AND (v.sender_id = $1 OR v.target_user_id = $1)
            ORDER BY v.updated_at DESC
            "#,
    )
    .bind(user_id)
    .fetch_all(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let matches: Vec<MatchDto> = rows
        .into_iter()
        .map(
            |(
                match_id,
                matched_user_id,
                first_name,
                last_name,
                profile_image,
                property_id,
                matched_at,
            )| {
                MatchDto {
                    match_id,
                    matched_user_id,
                    matched_user_name: format!("{} {}", first_name, last_name).trim().to_string(),
                    matched_user_image: profile_image,
                    property_id,
                    matched_at,
                }
            },
        )
        .collect();

    let response = ApiResponse {
        success: true,
        message: "Matches fetched successfully".to_string(),
        data: matches,
    };

    Ok((StatusCode::OK, Json(response)))
}
