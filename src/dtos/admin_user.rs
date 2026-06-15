use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUserFilter {
    pub search: Option<String>,
    pub role: Option<String>,
    pub associate_type: Option<String>,
    pub status: Option<String>,
    pub is_verified: Option<bool>,
    pub kyc_status: Option<String>,
    pub sort_by: Option<String>,
    pub sort_dir: Option<String>,
    
    #[serde(default)]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    10
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedUserList {
    pub users: Vec<serde_json::Value>,
    pub total: i64,
    pub page: u32,
    pub limit: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserRequest {
    pub status: Option<String>,
    pub user_role: Option<String>,
    pub associate_type: Option<String>,
    pub is_verified_broker: Option<bool>,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuspendUserRequest {
    pub reason: String, // 10-500 chars
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkActionRequest {
    pub user_ids: Vec<uuid::Uuid>,
    pub action: String, // suspend, reinstate, force-delete
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForceDeleteCounts {
    pub properties: i64,
    pub kyc: i64,
    pub chats: i64,
    pub messages: i64,
    pub news: i64,
    pub bookings: i64,
    pub reports: i64,
    pub reviews: i64,
    pub saved_rows: i64,
    pub notifications: i64,
    pub user: i64,
}
