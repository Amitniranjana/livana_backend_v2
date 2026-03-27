use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::{
        careers::{ApplicantDto, ApplyJobDto, CreateJobDto, CreateJobResponseDto},
        response::ApiResponse,
    },
    utils::{api_error::ApiError, auth_extractor::AuthenticationUser},
};

/// 1.1 Post Job (POST /api/v1/jobs)
pub async fn post_job(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Json(payload): Json<CreateJobDto>,
) -> Result<impl IntoResponse, ApiError> {
    let auth_user_id_uuid = Uuid::parse_str(&auth.user_id).unwrap_or_default();

    let job_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    sqlx::query(
        r#"
        INSERT INTO jobs (id, associate_id, title, description, location, salary_range, status, created_at, company_name, job_type, notice_period)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
    )
    .bind(job_id)
    .bind(auth_user_id_uuid)
    .bind(&payload.title)
    .bind(&payload.description)
    .bind(&payload.location)
    .bind(&payload.salary_range)
    .bind("ACTIVE")
    .bind(now)
    .bind(&payload.company_name)
    .bind(&payload.job_type)
    .bind(&payload.notice_period)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to create job: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Job posted successfully".to_string(),
        data: CreateJobResponseDto {
            job_id,
            status: "ACTIVE".to_string(),
            created_at: now,
        },
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// 1.2 Apply to Job (POST /api/v1/jobs/{job_id}/apply)
pub async fn apply_job(
    State(app_state): State<AppState>,
    Path(job_id): Path<Uuid>,
    auth: AuthenticationUser,
    Json(payload): Json<ApplyJobDto>,
) -> Result<impl IntoResponse, ApiError> {
    let auth_user_id_uuid = Uuid::parse_str(&auth.user_id).unwrap_or_default();

    // Enforce role: USER Only
    let user_role: (String,) = sqlx::query_as("SELECT user_role FROM users WHERE id = $1")
        .bind(auth_user_id_uuid)
        .fetch_one(&app_state.db)
        .await
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    if user_role.0.to_lowercase() != "user" {
        return Err(ApiError::Forbidden(
            "Only users can apply to jobs".to_string(),
        ));
    }

    let application_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    sqlx::query(
        r#"
        INSERT INTO job_applications (id, job_id, user_id, resume_url, cover_letter, applied_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(application_id)
    .bind(job_id)
    .bind(auth_user_id_uuid)
    .bind(&payload.resume_url)
    .bind(&payload.cover_letter)
    .bind(now)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to submit application: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Application submitted successfully".to_string(),
        data: json!({}),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// 1.3 Get Applicants (GET /api/v1/jobs/{job_id}/applicants)
pub async fn get_applicants(
    State(app_state): State<AppState>,
    Path(job_id): Path<Uuid>,
    auth: AuthenticationUser,
) -> Result<impl IntoResponse, ApiError> {
    let auth_user_id_uuid = Uuid::parse_str(&auth.user_id).unwrap_or_default();

    // 1. Enforce role: ASSOCIATE Only
    let user_role: (String,) = sqlx::query_as("SELECT user_role FROM users WHERE id = $1")
        .bind(auth_user_id_uuid)
        .fetch_one(&app_state.db)
        .await
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    if user_role.0.to_lowercase() != "associate" {
        return Err(ApiError::Forbidden(
            "Only associates can view applicants".to_string(),
        ));
    }

    // 2. Security Check: Ensure the ASSOCIATE requesting this is the actual owner/creator of the job_id
    let job_owner: Option<(Uuid,)> = sqlx::query_as("SELECT associate_id FROM jobs WHERE id = $1")
        .bind(job_id)
        .fetch_optional(&app_state.db)
        .await
        .map_err(|_| {
            ApiError::InternalServerError("Database error checking job owner".to_string())
        })?;

    match job_owner {
        Some((owner_id,)) => {
            if owner_id != auth_user_id_uuid {
                return Err(ApiError::Forbidden(
                    "You are not authorized to view applicants for this job".to_string(),
                ));
            }
        }
        None => return Err(ApiError::NotFound("Job not found".to_string())),
    }

    // 3. Fetch applicants
    let applicants_result: Result<
        Vec<(Uuid, Uuid, String, String, chrono::DateTime<chrono::Utc>)>,
        _,
    > = sqlx::query_as(
        r#"
        SELECT id, user_id, resume_url, cover_letter, applied_at
        FROM job_applications
        WHERE job_id = $1
        "#,
    )
    .bind(job_id)
    .fetch_all(&app_state.db)
    .await;

    let mut applicants = vec![];
    if let Ok(records) = applicants_result {
        for record in records {
            applicants.push(ApplicantDto {
                application_id: record.0,
                user_id: record.1,
                resume_url: record.2,
                cover_letter: record.3,
                applied_at: record.4,
            });
        }
    }

    let response = ApiResponse {
        success: true,
        message: "Applicants retrieved successfully".to_string(),
        data: applicants,
    };

    Ok((StatusCode::OK, Json(response)))
}
