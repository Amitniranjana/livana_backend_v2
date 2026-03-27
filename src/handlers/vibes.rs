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
    let vibe: Option<(Uuid, String)> =
        sqlx::query_as("SELECT target_user_id, status FROM vibes WHERE id = $1")
            .bind(vibe_id)
            .fetch_optional(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    match vibe {
        None => return Err(ApiError::NotFound("Vibe not found".to_string())),
        Some((target_id, _)) if target_id != user_id => {
            return Err(ApiError::Forbidden(
                "Only the target user can accept this vibe".to_string(),
            ));
        }
        Some((_, ref status)) if status != "pending" => {
            return Err(ApiError::BadRequest(format!(
                "Vibe has already been {}",
                status.to_lowercase()
            )));
        }
        _ => {}
    }

    // Update status to ACCEPTED
    sqlx::query("UPDATE vibes SET status = 'accepted', updated_at = NOW() WHERE id = $1")
        .bind(vibe_id)
        .execute(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Failed to accept vibe: {}", e)))?;

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
    let vibe: Option<(Uuid, String)> =
        sqlx::query_as("SELECT target_user_id, status FROM vibes WHERE id = $1")
            .bind(vibe_id)
            .fetch_optional(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    match vibe {
        None => return Err(ApiError::NotFound("Vibe not found".to_string())),
        Some((target_id, _)) if target_id != user_id => {
            return Err(ApiError::Forbidden(
                "Only the target user can reject this vibe".to_string(),
            ));
        }
        Some((_, ref status)) if status != "pending" => {
            return Err(ApiError::BadRequest(format!(
                "Vibe has already been {}",
                status.to_lowercase()
            )));
        }
        _ => {}
    }

    // Update status to REJECTED
    sqlx::query("UPDATE vibes SET status = 'rejected', updated_at = NOW() WHERE id = $1")
        .bind(vibe_id)
        .execute(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Failed to reject vibe: {}", e)))?;

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
