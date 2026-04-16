use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Request ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AddServiceRequest {
    pub service_name: String,
    pub category: String,
    pub price: i32, // hourly rate in INR
    pub description: String,
    pub experience: String,
    pub location: String,
}

/// Partial update — only provided fields are updated.
#[derive(Debug, Deserialize)]
pub struct UpdateServiceRequest {
    pub service_name: Option<String>,
    pub category: Option<String>,
    pub price: Option<i32>,
    pub description: Option<String>,
    pub experience: Option<String>,
    pub location: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ServicesQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ProvidersFilterQuery {
    pub service_type: Option<String>,
    pub sort_by: Option<String>, // "rating" | "price" | "experience"
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    #[allow(dead_code)]
    pub latitude: Option<f64>,
    #[allow(dead_code)]
    pub longitude: Option<f64>,
}

// ── Response ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ServiceResponse {
    pub service_id: Uuid,
    pub provider_id: Uuid,
    pub service_name: String,
    pub category: String,
    pub price: i32,
    pub description: String,
    pub experience: String,
    pub location: String,
    pub created_at: DateTime<Utc>,
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
    pub created_at: DateTime<Utc>,
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
    pub review_count: i64,
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
