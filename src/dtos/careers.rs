use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CreateJobDto {
    pub title: String,
    pub description: String,
    pub location: String,
    pub salary_range: String,
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
    pub applied_at: chrono::DateTime<chrono::Utc>,
}
