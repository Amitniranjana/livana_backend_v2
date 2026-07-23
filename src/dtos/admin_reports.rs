use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

// Issue 34
#[derive(Debug, Deserialize)]
pub struct AdminReportsQuery {
    pub status: Option<String>,
    pub property_id: Option<Uuid>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AdminReportListItem {
    pub id: Uuid,
    pub reporter_user: ReporterInfo,
    pub property_id: Uuid,
    pub property_snapshot: PropertySnapshot,
    pub reason: String,
    pub comment: Option<String>, // maps to description
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ReporterInfo {
    pub id: Uuid,
    pub name: String,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PropertySnapshot {
    pub title: String,
    pub owner_id: Uuid,
    pub owner_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AdminReportsListResponse {
    pub success: bool,
    pub data: AdminReportsData,
}

#[derive(Debug, Serialize)]
pub struct AdminReportsData {
    pub reports: Vec<AdminReportListItem>,
    pub pagination: Pagination,
}

#[derive(Debug, Serialize)]
pub struct Pagination {
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

// Issue 35
#[derive(Debug, Serialize)]
pub struct AdminReportDetailResponse {
    pub success: bool,
    pub data: AdminReportDetailData,
}

#[derive(Debug, Serialize)]
pub struct AdminReportDetailData {
    pub report: AdminReportListItem,
    pub report_history: Vec<AdminReportHistoryItem>, // other reports on this property
}

#[derive(Debug, Serialize)]
pub struct AdminReportHistoryItem {
    pub id: Uuid,
    pub reason: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

// Issue 36
#[derive(Debug, Deserialize)]
pub struct UpdateReportStatusRequest {
    pub status: String,
    pub resolution_note: Option<String>,
}
