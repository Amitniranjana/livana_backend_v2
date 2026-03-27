use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Request DTOs ──

#[derive(Debug, Deserialize)]
pub struct AddServiceRequest {
    pub service_name: String,
    pub category: String,
    pub price: i32,
    pub description: String,
    pub experience: String,
    pub location: String,
}

#[derive(Debug, Deserialize)]
pub struct ServicesListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ProvidersFilterQuery {
    pub service_type: Option<String>,
    pub sort_by: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

// ── Response DTOs ──

#[derive(Debug, Serialize)]
pub struct ServiceResponseData {
    pub service_id: Uuid,
    pub provider_id: Uuid,
    pub service_name: String,
    pub category: String,
    pub price: i32,
    pub description: String,
    pub experience: String,
    pub location: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ServiceItem {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub service_name: String,
    pub category: String,
    pub price: i32,
    pub description: String,
    pub experience: String,
    pub location: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ServicesListData {
    pub services: Vec<ServiceItem>,
    pub total_count: i64,
    pub current_page: i64,
    pub total_pages: i64,
}

#[derive(Debug, Serialize)]
pub struct ProviderItem {
    pub id: Uuid,
    pub name: String,
    pub service_type: String,
    pub rating: f64,
    pub review_count: i32,
    pub location: String,
    pub hourly_rate: f64,
    pub experience: String,
    pub is_verified: bool,
    pub availability: String,
    pub distance_km: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct ProvidersListData {
    pub providers: Vec<ProviderItem>,
    pub total_count: i64,
    pub current_page: i64,
    pub total_pages: i64,
}
