use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, NaiveDate, Utc};

#[derive(Debug, Deserialize)]
pub struct CrmLeadQuery {
    pub status: Option<String>,
    pub source: Option<String>,
    pub priority: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CrmLeadResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Option<Uuid>,
    pub project_name: Option<String>,
    pub source: String,
    pub source_detail: Option<String>,
    pub name: String,
    pub phone: String,
    pub email: Option<String>,
    pub budget_min: Option<i64>,
    pub budget_max: Option<i64>,
    pub requirement: Option<String>,
    pub location_preference: Option<String>,
    pub status: String,
    pub priority: String,
    pub notes: Option<String>,
    pub next_follow_up_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CrmLeadPagination {
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize)]
pub struct CrmLeadListResponse {
    pub leads: Vec<CrmLeadResponse>,
    pub pagination: CrmLeadPagination,
}

#[derive(Debug, Deserialize)]
pub struct CrmLeadPayload {
    pub project_id: Option<Uuid>,
    pub source: String,
    pub source_detail: Option<String>,
    pub name: String,
    pub phone: String,
    pub email: Option<String>,
    pub budget_min: Option<i64>,
    pub budget_max: Option<i64>,
    pub requirement: Option<String>,
    pub location_preference: Option<String>,
    pub status: Option<String>, // Can be optional on creation, defaults to 'new'
    pub priority: Option<String>, // Can be optional on creation, defaults to 'warm'
    pub notes: Option<String>,
    pub next_follow_up_date: Option<NaiveDate>,
}

#[derive(Debug, Deserialize)]
pub struct CrmLeadStatusUpdate {
    pub status: String,
}