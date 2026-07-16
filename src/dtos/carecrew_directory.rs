use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct CarecrewMemberResponse {
    pub id: Uuid,
    pub name: String,
    pub photo: Option<String>,
    pub city: Option<String>,
    pub services_offered: Option<serde_json::Value>,
    pub rating: Option<f64>,
    pub verified_kyc_status: String,
    // Only available if auth'd:
    pub phone: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CarecrewDirectoryResponse {
    pub success: bool,
    pub message: String,
    pub data: CarecrewDirectoryData,
}

#[derive(Debug, Serialize)]
pub struct CarecrewDirectoryData {
    pub members: Vec<CarecrewMemberResponse>,
    pub pagination: CarecrewDirectoryPagination,
}

#[derive(Debug, Serialize)]
pub struct CarecrewDirectoryPagination {
    pub total_count: i64,
    pub limit: i64,
    pub offset: i64,
}
