// src/handlers/google_auth.rs
//
// POST /auth/google
// Accepts a Google id_token, verifies it with Google's tokeninfo API,
// upserts the user in our DB, then issues a short-lived JWT.

use axum::{
    extract::{Json as ExtractJson, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::json;

use crate::app_state::AppState;
use crate::services::google_oauth_service::verify_google_id_token;
use crate::utils::auth::create_jwt;

// ---------------------------------------------------------------------------
// Request DTO
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct GoogleSigninRequest {
    pub id_token: String,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// POST /auth/google
///
/// 1. Verify the Google `id_token` against Google's tokeninfo endpoint.
/// 2. Look up or auto-create the user by their stable `google_id` (`sub`).
/// 3. Issue a short-lived (1-hour) JWT.
/// 4. Return `{ access_token, user }` where user contains id/email/name/picture.
pub async fn google_signin(
    State(state): State<AppState>,
    ExtractJson(payload): ExtractJson<GoogleSigninRequest>,
) -> impl IntoResponse {
    // ── Step 1: Verify the token with Google ──────────────────────────────
    let google_user = match verify_google_id_token(&payload.id_token, &state.google_client_id).await
    {
        Ok(u) => u,
        Err(e) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "message": format!("Invalid Google token: {}", e),
                    "data": null
                })),
            );
        }
    };

    // ── Step 2: Look up existing user, or create on first login ───────────
    let user_repo = &state.user_service.user_repository;

    let user = match user_repo.find_by_google_id(&google_user.google_id).await {
        Ok(Some(existing)) => existing,
        Ok(None) => {
            // First time this Google account has signed in → create user
            match user_repo
                .create_google_user(
                    &google_user.google_id,
                    &google_user.email,
                    &google_user.name,
                    google_user.picture.as_deref(),
                )
                .await
            {
                Ok(u) => u,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "success": false,
                            "message": format!("Failed to create user: {}", e),
                            "data": null
                        })),
                    );
                }
            }
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": format!("Database error: {}", e),
                    "data": null
                })),
            );
        }
    };

    // ── Step 3: Issue a JWT (15 days) ────────────────────────────────────
    // Security: standard expiry for long-lived sessions.
    let token = match create_jwt(&user.id.to_string(), &state.jwt_secret, 360) {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": format!("Failed to generate token: {}", e),
                    "data": null
                })),
            );
        }
    };

    // ── Step 4: Return access_token + user object ─────────────────────────
    let full_name = format!("{} {}", user.first_name, user.last_name)
        .trim()
        .to_string();

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Signed in with Google successfully",
            "data": {
                "access_token": token,
                "user": {
                    "id":      user.id,
                    "email":   user.email,
                    "name":    full_name,
                    "picture": user.profile_picture
                }
            }
        })),
    )
}
