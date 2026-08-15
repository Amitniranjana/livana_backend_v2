use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct AdminReportsQuery {
    #[serde(rename = "entityType")]
    pub entity_type: Option<String>, // USER, PROPERTY, COMMUNITY, POST
    pub status: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AdminReportListItem {
    pub id: Uuid,
    pub reporter_user: ReporterInfo,
    #[serde(rename = "entityType")]
    pub entity_type: String,
    #[serde(rename = "entityId")]
    pub entity_id: Uuid,
    #[serde(rename = "entitySnapshot")]
    pub entity_snapshot: Option<Value>,
    pub reason: String,
    pub comment: Option<String>,
    #[serde(rename = "adminNotes")]
    pub admin_notes: Option<String>,
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

#[derive(Debug, Serialize)]
pub struct AdminReportDetailResponse {
    pub success: bool,
    pub data: AdminReportDetailData,
}

#[derive(Debug, Serialize)]
pub struct AdminReportDetailData {
    pub report: AdminReportListItem,
    pub report_history: Vec<AdminReportHistoryItem>, // other reports on this entity
}

#[derive(Debug, Serialize)]
pub struct AdminReportHistoryItem {
    pub id: Uuid,
    pub reason: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateReportStatusRequest {
    pub status: String,
    #[serde(rename = "adminNotes")]
    pub admin_notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReportActionRequest {
    pub action: String, // suspend-user, delete-property, delete-news, dismiss
}
