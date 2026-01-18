use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct BrokerProfile {
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000", value_type = String)]
    pub user_id: Uuid,
    #[schema(example = "Elite Realty")]
    pub agency_name: String,
    #[schema(example = "LIC123456")]
    pub license_number: Option<String>,
    #[schema(example = "RERA7890")]
    pub rera_id: Option<String>,
    #[schema(example = "[\"New York\", \"Los Angeles\"]")]
    pub operating_cities: Option<Vec<String>>,
    #[schema(example = "[\"RENT\", \"SALE\"]")]
    pub deal_types: Option<Vec<String>>,
    #[schema(example = 5)]
    pub years_of_experience: Option<i32>,
    #[schema(example = "PENDING")]
    pub kyc_status: String,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String)]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateBrokerProfileRequest {
    #[schema(example = "Elite Realty")]
    pub agency_name: String,
    #[schema(example = "LIC123456")]
    pub license_number: Option<String>,
    #[schema(example = "RERA7890")]
    pub rera_id: Option<String>,
    #[schema(example = "[\"New York\", \"Los Angeles\"]")]
    pub operating_cities: Option<Vec<String>>,
    #[schema(example = "[\"RENT\", \"SALE\"]")]
    pub deal_types: Option<Vec<String>>,
    #[schema(example = 5)]
    pub years_of_experience: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[allow(dead_code)]
pub struct BrokerHomeStats {
    #[schema(example = 12)]
    pub total_properties: i32,
    #[schema(example = 5)]
    pub active_properties: i32,
    #[schema(example = 7)]
    pub sold_properties: i32,
    #[schema(example = 1500)]
    pub total_views: i32,
}
