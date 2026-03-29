use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// Used in GET /api/careers (lightweight, no description)
#[derive(Debug, Serialize)]
pub struct CareerListItem {
    pub job_id: Uuid,
    pub title: String,
    pub location: String,
    pub employment_type: String,
    pub experience: String,
    pub posted_at: DateTime<Utc>,
}

/// Used in GET /api/careers/{job_id} (full detail, includes description)
#[derive(Debug, Serialize)]
pub struct CareerDetailItem {
    pub job_id: Uuid,
    pub title: String,
    pub location: String,
    pub employment_type: String,
    pub experience: String,
    pub description: String,
    pub posted_at: DateTime<Utc>,
}
