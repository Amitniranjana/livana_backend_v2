use axum::{
    extract::{Json as ExtractJson, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
};
use serde_json::json;

use crate::app_state::AppState;
use crate::models::broker::{BrokerProfile, CreateBrokerProfileRequest};
use crate::utils::auth::decode_jwt;
use uuid::Uuid;

/// Broker Onboarding
/// POST /api/broker/onboarding
pub async fn onboarding(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    ExtractJson(payload): ExtractJson<CreateBrokerProfileRequest>,
) -> impl IntoResponse {
    // 1. Authenticate and Extract User
    let auth_header = match headers.get("Authorization") {
        Some(h) => h.to_str().unwrap_or(""),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"success": false, "message": "Missing Authorization Token"})),
            )
                .into_response();
        }
    };

    let token = auth_header.trim_start_matches("Bearer ");
    let claims = match decode_jwt(token, &app_state.jwt_secret) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"success": false, "message": "Invalid Token"})),
            )
                .into_response();
        }
    };

    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"success": false, "message": "Invalid User ID in Token"})),
            )
                .into_response();
        }
    };

    // 2. Verify User Role is BROKER
    let user_check = sqlx::query!("SELECT user_role FROM users WHERE id = $1", user_id)
        .fetch_optional(&app_state.db)
        .await;

    match user_check {
        Ok(Some(record)) => {
            if record.user_role.clone().unwrap_or_default().to_lowercase() != "broker" {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({"success": false, "message": "User is not a valid BROKER"})),
                )
                    .into_response();
            }
        }
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"success": false, "message": "User not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": format!("Database error: {}", e)})),
            )
                .into_response();
        }
    }

    // 3. Upsert Broker Profile
    // We use ON CONFLICT to allow updates if re-submitted (e.g. fixing rejected KYC)
    // Note: kyc_status logic could be refined (e.g. only reset to PENDING if not verified), but for onboarding we assume fresh start or update.
    let result = sqlx::query_as!(
        BrokerProfile,
        r#"
        INSERT INTO broker_profiles (user_id, agency_name, license_number, rera_id, operating_cities, deal_types, years_of_experience, kyc_status, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'PENDING', NOW())
        ON CONFLICT (user_id) DO UPDATE
        SET agency_name = $2,
            license_number = $3,
            rera_id = $4,
            operating_cities = $5,
            deal_types = $6,
            years_of_experience = $7,
            kyc_status = 'PENDING',
            updated_at = NOW()
        RETURNING *
        "#,
        user_id,
        payload.agency_name,
        payload.license_number,
        payload.rera_id,
        payload.operating_cities.as_deref(),
        payload.deal_types.as_deref(),
        payload.years_of_experience
    )
    .fetch_one(&app_state.db)
    .await;

    match result {
        Ok(profile) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Broker profile saved successfully",
                "data": profile
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "message": format!("Failed to save profile: {}", e)
            })),
        )
            .into_response(),
    }
}

/// Get Broker Profile
/// GET /api/broker/profile
pub async fn get_profile(
    State(app_state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // 1. Authenticate
    let auth_header = match headers.get("Authorization") {
        Some(h) => h.to_str().unwrap_or(""),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"success": false, "message": "Missing Authorization Token"})),
            )
                .into_response();
        }
    };

    let token = auth_header.trim_start_matches("Bearer ");
    let claims = match decode_jwt(token, &app_state.jwt_secret) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"success": false, "message": "Invalid Token"})),
            )
                .into_response();
        }
    };

    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"success": false, "message": "Invalid User ID in Token"})),
            )
                .into_response();
        }
    };

    // 2. Fetch Profile + User Name (for header)
    // We join with users table to get name if needed, or just return profile.
    // The prompt mentions "Welcome Header with broker name", so fetching user details might be good,
    // but the task asks for "Broker Profile" primarily.
    // Let's fetch just the profile for the specific broker endpoint, or the combined data if relevant.
    // Given the prompt "Return 404 if profile doesn't exist", we focus on broker_profiles.

    // We can just fetch BrokerProfile.
    let profile = sqlx::query_as!(
        BrokerProfile,
        "SELECT * FROM broker_profiles WHERE user_id = $1",
        user_id
    )
    .fetch_optional(&app_state.db)
    .await;

    match profile {
        Ok(Some(p)) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Broker profile retrieved successfully",
                "data": p
            })),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "message": "Broker profile not found. Please complete onboarding.",
                "code": "PROFILE_NOT_FOUND" // Frontend signal to redirect
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "message": format!("Database error: {}", e)
            })),
        )
            .into_response(),
    }
}
