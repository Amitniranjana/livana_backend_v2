use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CreateJobDto {
    pub title: String,
    pub description: String,
    pub location: String,
    pub salary_range: String,
    pub company_name: Option<String>,
    pub job_type: Option<String>,
    pub notice_period: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct CreateJobResponseDto {
    pub job_id: Uuid,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ApplyJobDto {
    pub resume_url: String,
    pub cover_letter: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ApplicantDto {
    pub application_id: Uuid,
    pub user_id: Uuid,
    pub resume_url: String,
    pub cover_letter: String,
    pub status: String,
    pub applied_at: chrono::DateTime<chrono::Utc>,
}

/// DTO for updating application status
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct UpdateApplicationStatusDto {
    pub status: String,
}

/// DTO for job list items (lightweight — no full description)
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct JobListDto {
    pub id: Uuid,
    pub title: String,
    pub company_name: Option<String>,
    pub location: Option<String>,
    pub salary_range: Option<String>,
    pub job_type: Option<String>,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// DTO for full job detail
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct JobDetailDto {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub company_name: Option<String>,
    pub location: Option<String>,
    pub salary_range: Option<String>,
    pub job_type: Option<String>,
    pub notice_period: Option<String>,
    pub status: String,
    pub created_by: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// DTO for editing a job listing
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct UpdateJobDto {
    pub title: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub salary_range: Option<String>,
    pub company_name: Option<String>,
    pub job_type: Option<String>,
    pub notice_period: Option<String>,
    pub status: Option<String>,
}

/// Query params for job listing with pagination/filters
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct JobListQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub location: Option<String>,
    pub job_type: Option<String>,
    pub status: Option<String>,
}
