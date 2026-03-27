// src/handlers/saved_properties.rs
//
// Module 5: Saved Property API
//   5.1  POST   /api/v1/properties/{id}/save
//   5.2  DELETE /api/v1/properties/{id}/save
//   5.3  GET    /api/v1/users/me/saved-properties

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
        saved_properties::{PropertyDto, SavedPropertyResponseDto},
    },
    utils::{api_error::ApiError, auth_extractor::AuthenticationUser},
};

// ---------------------------------------------------------------------------
// 5.1  POST /api/v1/properties/{id}/save  — Save / bookmark a property
// ---------------------------------------------------------------------------

pub async fn save_property(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(property_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    // Verify the property exists
    let exists: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM properties WHERE id = $1 AND status = 'active'")
            .bind(property_id)
            .fetch_optional(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    if exists.is_none() {
        return Err(ApiError::NotFound("Property not found".to_string()));
    }

    // Insert or do nothing if already saved (graceful duplicate handling)
    sqlx::query(
        r#"
        INSERT INTO saved_properties (id, user_id, property_id, created_at)
        VALUES ($1, $2, $3, NOW())
        ON CONFLICT (user_id, property_id) DO NOTHING
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(property_id)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to save property: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Property saved successfully".to_string(),
        data: SavedPropertyResponseDto {
            property_id,
            is_saved: true,
        },
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// 5.2  DELETE /api/v1/properties/{id}/save  — Remove saved property
// ---------------------------------------------------------------------------

pub async fn unsave_property(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(property_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    // Delete — if it wasn't saved, this is a no-op (graceful)
    sqlx::query("DELETE FROM saved_properties WHERE user_id = $1 AND property_id = $2")
        .bind(user_id)
        .bind(property_id)
        .execute(&app_state.db)
        .await
        .map_err(|e| {
            ApiError::InternalServerError(format!("Failed to remove saved property: {}", e))
        })?;

    let response = ApiResponse {
        success: true,
        message: "Property removed from saved list".to_string(),
        data: SavedPropertyResponseDto {
            property_id,
            is_saved: false,
        },
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// 5.3  GET /api/v1/users/me/saved-properties  — List saved properties
// ---------------------------------------------------------------------------

pub async fn get_saved_properties(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    let rows: Vec<(
        Uuid,
        String,
        Option<i64>,
        Option<String>,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        r#"
            SELECT p.id, p.title, p.price, p.city AS location, p.created_at
            FROM saved_properties sp
            JOIN properties p ON sp.property_id = p.id
            WHERE sp.user_id = $1 AND p.status = 'active'
            ORDER BY sp.created_at DESC
            "#,
    )
    .bind(user_id)
    .fetch_all(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let properties: Vec<PropertyDto> = rows
        .into_iter()
        .map(|(id, title, price, location, created_at)| PropertyDto {
            id,
            title,
            price,
            location,
            created_at,
        })
        .collect();

    let response = ApiResponse {
        success: true,
        message: "Saved properties fetched successfully".to_string(),
        data: properties,
    };

    Ok((StatusCode::OK, Json(response)))
}
