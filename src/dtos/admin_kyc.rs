use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct AdminKycListQuery {
    pub status: Option<String>, // 'pending', 'verified', 'rejected', 'all'
    pub user_role: Option<String>, // 'builder', 'broker', 'associate', 'user'
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AdminKycListItemResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub role: Option<String>,
    pub submitted_docs_summary: String, // e.g. "Govt ID, Experience Doc"
    pub status: String,
    pub submitted_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct AdminKycListResponse {
    pub success: bool,
    pub message: String,
    pub data: AdminKycListData,
}

#[derive(Debug, Serialize)]
pub struct AdminKycListData {
    pub kyc_records: Vec<AdminKycListItemResponse>,
    pub pagination: AdminKycPagination,
}

#[derive(Debug, Serialize)]
pub struct AdminKycPagination {
    pub total_count: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize)]
pub struct AdminKycDetailResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub role: Option<String>,
    pub full_name: String,
    pub mobile_number: String,
    pub email_id: String,
    pub gender: Option<String>,
    pub date_of_birth: Option<chrono::NaiveDate>,
    pub profile_picture_url: Option<String>,
    pub address: Option<String>, // combined address or just street_address
    pub govt_id_type: String,
    pub govt_id_number: String,
    pub govt_id_document_url: String,
    pub company_name: Option<String>,
    pub services: Option<serde_json::Value>,
    pub experience_document_url: Option<String>,
    pub verification_status: String,
    pub submitted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub verified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub rejection_reason: Option<String>,
    
    // Linked profile details if available
    pub linked_profile: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct AdminKycRejectRequest {
    pub reason: String,
}
