// src/handlers/language.rs
//
// Module 11: Language API
//   11.1  GET   /api/v1/languages           — Get available languages
//   11.2  PATCH /api/v1/users/me/language   — Set preferred language

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::{
        language::{LanguageDto, SetLanguageDto},
        response::ApiResponse,
    },
    utils::{api_error::ApiError, auth_extractor::AuthenticationUser},
};

// ---------------------------------------------------------------------------
// 11.1  GET /api/v1/languages — Get all available languages
// ---------------------------------------------------------------------------

pub async fn get_languages(
    State(app_state): State<AppState>,
    _auth: AuthenticationUser,
) -> Result<impl IntoResponse, ApiError> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT code, name FROM languages ORDER BY name ASC",
    )
    .fetch_all(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let languages: Vec<LanguageDto> = rows
        .into_iter()
        .map(|(code, name)| LanguageDto { code, name })
        .collect();

    let response = ApiResponse {
        success: true,
        message: "Languages fetched successfully".to_string(),
        data: languages,
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// 11.2  PATCH /api/v1/users/me/language — Set user's preferred language
// ---------------------------------------------------------------------------

pub async fn set_preferred_language(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Json(payload): Json<SetLanguageDto>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    let code = payload.language_code.trim().to_lowercase();

    if code.is_empty() {
        return Err(ApiError::BadRequest(
            "language_code cannot be empty".to_string(),
        ));
    }

    // Verify the language code exists
    let exists: Option<String> =
        sqlx::query_scalar("SELECT code FROM languages WHERE code = $1")
            .bind(&code)
            .fetch_optional(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    if exists.is_none() {
        return Err(ApiError::NotFound(
            "Language not found. Use GET /api/v1/languages to see available options.".to_string(),
        ));
    }

    // Update the user's preferred language
    sqlx::query("UPDATE users SET preferred_language = $1, updated_at = NOW() WHERE id = $2")
        .bind(&code)
        .bind(user_id)
        .execute(&app_state.db)
        .await
        .map_err(|e| {
            ApiError::InternalServerError(format!("Failed to update language preference: {}", e))
        })?;

    let response = ApiResponse {
        success: true,
        message: "Language preference updated successfully".to_string(),
        data: serde_json::json!({ "language_code": code }),
    };

    Ok((StatusCode::OK, Json(response)))
}
