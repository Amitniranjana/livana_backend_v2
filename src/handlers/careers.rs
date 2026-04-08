use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::{
        careers::{
            ApplicantDto, ApplyJobDto, CreateJobDto, CreateJobResponseDto, JobDetailDto,
            JobListDto, JobListQuery, UpdateJobDto,
        },
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

/// 1.2 Get All Jobs (GET /api/v1/jobs)
/// Supports pagination (?page=1&limit=10) and filters (?location=Ahmedabad&job_type=full-time)
#[allow(dead_code)]
pub async fn list_jobs(
    State(app_state): State<AppState>,
    Query(q): Query<JobListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let page = q.page.unwrap_or(1).max(1);
    let limit = q.limit.unwrap_or(10).clamp(1, 50);
    let offset = (page - 1) * limit;

    // Build filtered count query
    let mut count_sql = String::from("SELECT COUNT(*) FROM jobs WHERE 1=1");
    let mut query_sql = String::from(
        r#"SELECT id, title, company_name, location, salary_range, job_type, status, created_at
           FROM jobs WHERE 1=1"#,
    );

    // We'll add conditions as text and bind manually
    let mut conditions = Vec::new();
    let mut bind_idx = 1u32;

    if let Some(ref loc) = q.location {
        if !loc.is_empty() {
            conditions.push(format!(" AND LOWER(location) LIKE LOWER(${})", bind_idx));
            bind_idx += 1;
            let _ = loc; // used below
        }
    }
    if let Some(ref jt) = q.job_type {
        if !jt.is_empty() {
            conditions.push(format!(" AND LOWER(job_type) = LOWER(${})", bind_idx));
            bind_idx += 1;
            let _ = jt;
        }
    }

    let status_filter = q.status.as_deref().unwrap_or("ACTIVE");
    conditions.push(format!(" AND status = ${}", bind_idx));
    bind_idx += 1;
    let _ = status_filter;

    let conditions_str: String = conditions.join("");
    count_sql.push_str(&conditions_str);
    query_sql.push_str(&conditions_str);
    query_sql.push_str(&format!(
        " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
        bind_idx,
        bind_idx + 1
    ));

    // Execute count query
    let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
    if let Some(ref loc) = q.location {
        if !loc.is_empty() {
            count_query = count_query.bind(format!("%{}%", loc));
        }
    }
    if let Some(ref jt) = q.job_type {
        if !jt.is_empty() {
            count_query = count_query.bind(jt.as_str());
        }
    }
    count_query = count_query.bind(status_filter);

    let total_count = count_query
        .fetch_one(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Count query error: {}", e)))?;

    // Execute data query
    let mut data_query = sqlx::query_as::<_, (
        Uuid,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        chrono::DateTime<chrono::Utc>,
    )>(&query_sql);

    if let Some(ref loc) = q.location {
        if !loc.is_empty() {
            data_query = data_query.bind(format!("%{}%", loc));
        }
    }
    if let Some(ref jt) = q.job_type {
        if !jt.is_empty() {
            data_query = data_query.bind(jt.as_str());
        }
    }
    data_query = data_query.bind(status_filter);
    data_query = data_query.bind(limit);
    data_query = data_query.bind(offset);

    let rows = data_query
        .fetch_all(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("List query error: {}", e)))?;

    let jobs: Vec<JobListDto> = rows
        .into_iter()
        .map(
            |(id, title, company_name, location, salary_range, job_type, status, created_at)| {
                JobListDto {
                    id,
                    title,
                    company_name,
                    location,
                    salary_range,
                    job_type,
                    status,
                    created_at,
                }
            },
        )
        .collect();

    let total_pages = ((total_count as f64) / (limit as f64)).ceil() as i64;

    let response = json!({
        "success": true,
        "message": "Jobs retrieved successfully",
        "data": {
            "jobs": jobs,
            "pagination": {
                "total_count": total_count,
                "current_page": page,
                "total_pages": total_pages.max(1),
                "limit": limit
            }
        }
    });

    Ok((StatusCode::OK, Json(response)))
}

/// 1.3 Get Job Description (GET /api/v1/jobs/{job_id})
#[allow(dead_code)]
pub async fn get_job_detail(
    State(app_state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let row: Option<(
        Uuid,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        Uuid,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        r#"
        SELECT id, title, description, company_name, location, salary_range,
               job_type, notice_period, status, associate_id, created_at
        FROM jobs
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    match row {
        Some((
            id,
            title,
            description,
            company_name,
            location,
            salary_range,
            job_type,
            notice_period,
            status,
            associate_id,
            created_at,
        )) => {
            let detail = JobDetailDto {
                id,
                title,
                description,
                company_name,
                location,
                salary_range,
                job_type,
                notice_period,
                status,
                created_by: associate_id,
                created_at,
            };

            let response = ApiResponse {
                success: true,
                message: "Job details retrieved successfully".to_string(),
                data: detail,
            };

            Ok((StatusCode::OK, Json(response)))
        }
        None => Err(ApiError::NotFound("Job not found".to_string())),
    }
}

/// 1.4 Edit Job Listing (PUT /api/v1/jobs/{job_id})
/// Only the creator or admin can edit
#[allow(dead_code)]
pub async fn edit_job(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(job_id): Path<Uuid>,
    Json(payload): Json<UpdateJobDto>,
) -> Result<impl IntoResponse, ApiError> {
    let auth_user_id_uuid = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    // 1. Fetch the job and verify ownership
    let job_owner: Option<(Uuid,)> =
        sqlx::query_as("SELECT associate_id FROM jobs WHERE id = $1")
            .bind(job_id)
            .fetch_optional(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    match job_owner {
        None => return Err(ApiError::NotFound("Job not found".to_string())),
        Some((owner_id,)) if owner_id != auth_user_id_uuid => {
            // Check if user is admin
            let user_role: Option<(String,)> =
                sqlx::query_as("SELECT user_role FROM users WHERE id = $1")
                    .bind(auth_user_id_uuid)
                    .fetch_optional(&app_state.db)
                    .await
                    .map_err(|e| {
                        ApiError::InternalServerError(format!("Database error: {}", e))
                    })?;

            let is_admin = user_role
                .map(|(role,)| role.to_lowercase() == "admin")
                .unwrap_or(false);

            if !is_admin {
                return Err(ApiError::Forbidden(
                    "Only the job creator or admin can edit this job".to_string(),
                ));
            }
        }
        _ => {} // Owner matches — proceed
    }

    // 2. Update the job with COALESCE to keep existing values for NULL fields
    sqlx::query(
        r#"
        UPDATE jobs SET
            title         = COALESCE($2, title),
            description   = COALESCE($3, description),
            location      = COALESCE($4, location),
            salary_range  = COALESCE($5, salary_range),
            company_name  = COALESCE($6, company_name),
            job_type      = COALESCE($7, job_type),
            notice_period = COALESCE($8, notice_period),
            status        = COALESCE($9, status),
            updated_at    = NOW()
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .bind(&payload.title)
    .bind(&payload.description)
    .bind(&payload.location)
    .bind(&payload.salary_range)
    .bind(&payload.company_name)
    .bind(&payload.job_type)
    .bind(&payload.notice_period)
    .bind(&payload.status)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to update job: {}", e)))?;

    // 3. Fetch updated job and return
    let updated: (
        Uuid,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        Uuid,
        chrono::DateTime<chrono::Utc>,
    ) = sqlx::query_as(
        r#"
        SELECT id, title, description, company_name, location, salary_range,
               job_type, notice_period, status, associate_id, created_at
        FROM jobs WHERE id = $1
        "#,
    )
    .bind(job_id)
    .fetch_one(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let detail = JobDetailDto {
        id: updated.0,
        title: updated.1,
        description: updated.2,
        company_name: updated.3,
        location: updated.4,
        salary_range: updated.5,
        job_type: updated.6,
        notice_period: updated.7,
        status: updated.8,
        created_by: updated.9,
        created_at: updated.10,
    };

    let response = ApiResponse {
        success: true,
        message: "Job updated successfully".to_string(),
        data: detail,
    };

    Ok((StatusCode::OK, Json(response)))
}

/// 1.5 Apply to Job (POST /api/v1/jobs/{job_id}/apply)
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

/// 1.6 Get Applicants (GET /api/v1/jobs/{job_id}/applicants)
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

