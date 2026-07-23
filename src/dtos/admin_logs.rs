use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Deserialize)]
pub struct AdminLogsQuery {
    pub admin_id: Option<String>,
    pub action_type: Option<String>,
    pub target_type: Option<String>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AdminLogResponseItem {
    pub id: Uuid,
    pub admin_id: String,
    pub admin_name: Option<String>,
    pub action_type: String,
    pub target_type: String,
    pub target_id: Option<Uuid>,
    pub details: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AdminLogsListResponse {
    pub success: bool,
    pub data: AdminLogsData,
}

#[derive(Debug, Serialize)]
pub struct AdminLogsData {
    pub total: i64,
    pub logs: Vec<AdminLogResponseItem>,
}
