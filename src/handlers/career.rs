use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::career::{CareerDetailItem, CareerListItem},
    dtos::response::ApiResponse,
    utils::api_error::ApiError,
};

// ─────────────────────────────────────────────────────────────────────────────
// GET /api/careers
// Returns all active jobs sorted by latest (created_at DESC)
// Response is lightweight — no description field
// ─────────────────────────────────────────────────────────────────────────────
pub async fn list_careers(
    State(app_state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<CareerListItem>>>, ApiError> {
    let rows = sqlx::query!(
        r#"
        SELECT id, title, location, employment_type, experience, created_at
        FROM careers
        WHERE is_active = TRUE
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(e.to_string()))?;

    let jobs: Vec<CareerListItem> = rows
        .into_iter()
        .map(|r| CareerListItem {
            job_id: r.id,
            title: r.title,
            location: r.location,
            employment_type: r.employment_type,
            experience: r.experience,
            posted_at: r.created_at,
        })
        .collect();

    Ok(Json(ApiResponse {
        success: true,
        message: "Careers retrieved successfully".to_string(),
        data: jobs,
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// GET /api/careers/{job_id}
// Returns full job details for a single active job
// Returns 404 if job not found OR if job is inactive
// ─────────────────────────────────────────────────────────────────────────────
pub async fn get_career_detail(
    State(app_state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<ApiResponse<CareerDetailItem>>, ApiError> {
    let row = sqlx::query!(
        r#"
        SELECT id, title, description, location, employment_type, experience, is_active, created_at
        FROM careers
        WHERE id = $1
        "#,
        job_id,
    )
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(e.to_string()))?;

    match row {
        // Job not found at all → 404
        None => Err(ApiError::CustomError(
            StatusCode::NOT_FOUND,
            "Job not found".to_string(),
            "NOT_FOUND".to_string(),
        )),
        // Job exists but is inactive → also 404 (do not reveal inactive jobs)
        Some(r) if !r.is_active => Err(ApiError::CustomError(
            StatusCode::NOT_FOUND,
            "Job not found".to_string(),
            "NOT_FOUND".to_string(),
        )),
        // Job found and active → return full details
        Some(r) => Ok(Json(ApiResponse {
            success: true,
            message: "Job details retrieved successfully".to_string(),
            data: CareerDetailItem {
                job_id: r.id,
                title: r.title,
                location: r.location,
                employment_type: r.employment_type,
                experience: r.experience,
                description: r.description,
                posted_at: r.created_at,
            },
        })),
    }
}
