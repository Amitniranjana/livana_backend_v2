use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct BuilderProfile {
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000", value_type = String)]
    pub user_id: Uuid,
    #[schema(example = "Skyline Builders Pvt Ltd")]
    pub company_name: String,
    #[schema(example = "RERA/TN/2024/001234")]
    pub rera_id: Option<String>,
    #[schema(example = "33AAAAA0000A1Z5")]
    pub gst_number: Option<String>,
    #[schema(example = "U45201TN2015PTC012345")]
    pub cin_number: Option<String>,
    #[schema(example = 2015)]
    pub established_year: Option<i32>,
    #[schema(example = "[\"Chennai\", \"Coimbatore\"]")]
    pub operating_cities: Option<Vec<String>>,
    #[schema(example = "[\"Residential\", \"Commercial\", \"Villa\"]")]
    pub project_categories: Option<Vec<String>>,
    #[schema(example = 10)]
    pub years_of_experience: Option<i32>,
    #[schema(example = 24)]
    pub total_projects_completed: Option<i32>,
    #[schema(example = "12 Anna Salai, Chennai")]
    pub office_address: Option<String>,
    #[schema(example = "https://skylinebuilders.com")]
    pub website_url: Option<String>,
    #[schema(example = "https://example.com/logo.png")]
    pub logo_url: Option<String>,
    #[schema(example = "Award winning builder in South India")]
    pub description: Option<String>,
    #[schema(example = "PENDING")]
    pub kyc_status: String,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String)]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateBuilderProfileRequest {
    #[schema(example = "Skyline Builders Pvt Ltd")]
    pub company_name: String,
    #[schema(example = "RERA/TN/2024/001234")]
    pub rera_id: Option<String>,
    #[schema(example = "33AAAAA0000A1Z5")]
    pub gst_number: Option<String>,
    #[schema(example = "U45201TN2015PTC012345")]
    pub cin_number: Option<String>,
    #[schema(example = 2015)]
    pub established_year: Option<i32>,
    #[schema(example = "[\"Chennai\", \"Coimbatore\"]")]
    pub operating_cities: Option<Vec<String>>,
    #[schema(example = "[\"Residential\", \"Commercial\", \"Villa\"]")]
    pub project_categories: Option<Vec<String>>,
    #[schema(example = 10)]
    pub years_of_experience: Option<i32>,
    #[schema(example = 24)]
    pub total_projects_completed: Option<i32>,
    #[schema(example = "12 Anna Salai, Chennai")]
    pub office_address: Option<String>,
    #[schema(example = "https://skylinebuilders.com")]
    pub website_url: Option<String>,
    #[schema(example = "https://example.com/logo.png")]
    pub logo_url: Option<String>,
    #[schema(example = "Award winning builder in South India")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateBuilderProfileRequest {
    #[schema(example = "Skyline Builders Pvt Ltd")]
    pub company_name: Option<String>,
    #[schema(example = "RERA/TN/2024/001234")]
    pub rera_id: Option<String>,
    #[schema(example = "33AAAAA0000A1Z5")]
    pub gst_number: Option<String>,
    #[schema(example = "U45201TN2015PTC012345")]
    pub cin_number: Option<String>,
    #[schema(example = 2015)]
    pub established_year: Option<i32>,
    #[schema(example = "[\"Chennai\", \"Coimbatore\"]")]
    pub operating_cities: Option<Vec<String>>,
    #[schema(example = "[\"Residential\", \"Commercial\", \"Villa\"]")]
    pub project_categories: Option<Vec<String>>,
    #[schema(example = 10)]
    pub years_of_experience: Option<i32>,
    #[schema(example = 24)]
    pub total_projects_completed: Option<i32>,
    #[schema(example = "12 Anna Salai, Chennai")]
    pub office_address: Option<String>,
    #[schema(example = "https://skylinebuilders.com")]
    pub website_url: Option<String>,
    #[schema(example = "https://example.com/logo.png")]
    pub logo_url: Option<String>,
    #[schema(example = "Award winning builder in South India")]
    pub description: Option<String>,
}
