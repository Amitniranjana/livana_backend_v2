use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Query parameters for filtering properties.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct PropertyFilterParams {
    /// Property type: residential, commercial, acres
    #[serde(rename = "type")]
    pub property_type: Option<String>,
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
    pub city: Option<String>,
    /// 1-based page number (default: 1)
    pub page: Option<i64>,
    /// Items per page (default: 10, max: 50)
    pub limit: Option<i64>,
}

/// Simplified property item in filter results.
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct FilteredPropertyDto {
    pub id: Uuid,
    pub title: String,
    pub price: Option<i64>,
    pub location: Option<String>,
    #[serde(rename = "type")]
    pub property_type: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Pagination metadata.
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct PaginationMeta {
    pub total_count: i64,
    pub current_page: i64,
    pub total_pages: i64,
}

/// Paginated response wrapper.
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct PaginatedPropertiesData {
    pub properties: Vec<FilteredPropertyDto>,
    pub total_count: i64,
    pub current_page: i64,
    pub total_pages: i64,
}
