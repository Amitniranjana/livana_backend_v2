use axum::{
    extract::{Json as ExtractJson, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
};
use serde_json::json;

use crate::app_state::AppState;
use crate::models::builder::{BuilderProfile, CreateBuilderProfileRequest, UpdateBuilderProfileRequest};
use crate::utils::auth::decode_jwt;
use uuid::Uuid;

/// Builder Onboarding
/// POST /api/builder/onboarding
pub async fn onboarding(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    ExtractJson(payload): ExtractJson<CreateBuilderProfileRequest>,
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

    // 2. Verify User Role is BUILDER
    let user_check: Option<String> = sqlx::query_scalar("SELECT user_role FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&app_state.db)
        .await
        .unwrap_or(None);

    if let Some(role) = user_check {
        if role.to_lowercase() != "builder" {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"success": false, "message": "User is not a valid BUILDER"})),
            )
                .into_response();
        }
    } else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "User not found"})),
        )
            .into_response();
    }

    // 3. Upsert Builder Profile
    let result = sqlx::query_as::<_, BuilderProfile>(
        r#"
        INSERT INTO builder_profiles (
            user_id, company_name, rera_id, gst_number, cin_number, established_year, 
            operating_cities, project_categories, years_of_experience, total_projects_completed, 
            office_address, website_url, logo_url, description, kyc_status, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, 'PENDING', NOW())
        ON CONFLICT (user_id) DO UPDATE
        SET company_name = $2,
            rera_id = $3,
            gst_number = $4,
            cin_number = $5,
            established_year = $6,
            operating_cities = $7,
            project_categories = $8,
            years_of_experience = $9,
            total_projects_completed = $10,
            office_address = $11,
            website_url = $12,
            logo_url = $13,
            description = $14,
            kyc_status = 'PENDING',
            updated_at = NOW()
        RETURNING *
        "#
    )
    .bind(user_id)
    .bind(payload.company_name)
    .bind(payload.rera_id)
    .bind(payload.gst_number)
    .bind(payload.cin_number)
    .bind(payload.established_year)
    .bind(payload.operating_cities)
    .bind(payload.project_categories)
    .bind(payload.years_of_experience)
    .bind(payload.total_projects_completed)
    .bind(payload.office_address)
    .bind(payload.website_url)
    .bind(payload.logo_url)
    .bind(payload.description)
    .fetch_one(&app_state.db)
    .await;

    match result {
        Ok(profile) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Builder profile saved successfully",
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

/// Get Builder Profile
/// GET /api/builder/profile
pub async fn get_profile(
    State(app_state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
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

    let profile = sqlx::query_as::<_, BuilderProfile>(
        "SELECT * FROM builder_profiles WHERE user_id = $1"
    )
    .bind(user_id)
    .fetch_optional(&app_state.db)
    .await;

    match profile {
        Ok(Some(p)) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Builder profile retrieved successfully",
                "data": p
            })),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "message": "Builder profile not found. Please complete onboarding.",
                "code": "PROFILE_NOT_FOUND"
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

/// Update Builder Profile (Partial)
/// PUT /api/builder/profile
pub async fn update_profile(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    ExtractJson(payload): ExtractJson<UpdateBuilderProfileRequest>,
) -> impl IntoResponse {
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
    
    // Ensure profile exists first
    let exists: Option<i32> = sqlx::query_scalar("SELECT 1 AS exists FROM builder_profiles WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(&app_state.db)
        .await
        .unwrap_or(None);
        
    match exists {
        Some(_) => {},
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "message": "Builder profile not found. Please complete onboarding.",
                    "code": "PROFILE_NOT_FOUND"
                })),
            ).into_response();
        },
    }

    let result = sqlx::query_as::<_, BuilderProfile>(
        r#"
        UPDATE builder_profiles
        SET company_name = COALESCE($2, company_name),
            rera_id = COALESCE($3, rera_id),
            gst_number = COALESCE($4, gst_number),
            cin_number = COALESCE($5, cin_number),
            established_year = COALESCE($6, established_year),
            operating_cities = COALESCE($7, operating_cities),
            project_categories = COALESCE($8, project_categories),
            years_of_experience = COALESCE($9, years_of_experience),
            total_projects_completed = COALESCE($10, total_projects_completed),
            office_address = COALESCE($11, office_address),
            website_url = COALESCE($12, website_url),
            logo_url = COALESCE($13, logo_url),
            description = COALESCE($14, description),
            updated_at = NOW()
        WHERE user_id = $1
        RETURNING *
        "#
    )
    .bind(user_id)
    .bind(payload.company_name)
    .bind(payload.rera_id)
    .bind(payload.gst_number)
    .bind(payload.cin_number)
    .bind(payload.established_year)
    .bind(payload.operating_cities)
    .bind(payload.project_categories)
    .bind(payload.years_of_experience)
    .bind(payload.total_projects_completed)
    .bind(payload.office_address)
    .bind(payload.website_url)
    .bind(payload.logo_url)
    .bind(payload.description)
    .fetch_one(&app_state.db)
    .await;

    match result {
        Ok(profile) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Builder profile updated successfully",
                "data": profile
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "message": format!("Failed to update profile: {}", e)
            })),
        )
            .into_response(),
    }
}
