use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct BuilderProject {
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub id: Uuid,
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub user_id: Uuid,
    #[schema(example = "Skyline Heights")]
    pub project_name: String,
    #[schema(example = "Residential")]
    pub project_type: String,
    #[schema(example = "upcoming")]
    pub status: String,
    #[schema(example = "Premium 2/3 BHK apartments with clubhouse.")]
    pub description: Option<String>,
    #[schema(example = "Chennai")]
    pub city: String,
    #[schema(example = "OMR")]
    pub locality: String,
    #[schema(example = "Near ELCOT SEZ, OMR")]
    pub address: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    #[schema(example = "RERA/TN/2025/000456")]
    pub rera_id: Option<String>,
    #[schema(example = 240)]
    pub total_units: Option<i32>,
    #[schema(example = 3)]
    pub total_towers: Option<i32>,
    #[schema(example = "[\"2BHK\", \"3BHK\"]")]
    pub unit_configurations: Option<Vec<String>>,
    #[schema(example = 6500000)]
    pub price_range_min: Option<i64>,
    #[schema(example = 15000000)]
    pub price_range_max: Option<i64>,
    #[schema(example = 950)]
    pub area_range_min_sqft: Option<i32>,
    #[schema(example = 2100)]
    pub area_range_max_sqft: Option<i32>,
    pub possession_date: Option<chrono::NaiveDate>,
    pub launch_date: Option<chrono::NaiveDate>,
    #[schema(example = "[\"Clubhouse\", \"Gym\"]")]
    pub amenities: Option<Vec<String>>,
    pub nearby_places: Option<serde_json::Value>,
    pub images: Option<serde_json::Value>,
    pub brochure_url: Option<String>,
    pub video_url: Option<String>,
    pub master_plan_image_url: Option<String>,
    pub floor_plans: Option<serde_json::Value>,
    pub views_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateProjectRequest {
    #[schema(example = "Skyline Heights")]
    pub project_name: String,
    #[schema(example = "Residential")]
    pub project_type: String,
    #[schema(example = "upcoming")]
    pub status: String,
    #[schema(example = "Premium 2/3 BHK apartments with clubhouse.")]
    pub description: Option<String>,
    #[schema(example = "Chennai")]
    pub city: String,
    #[schema(example = "OMR")]
    pub locality: String,
    #[schema(example = "Near ELCOT SEZ, OMR")]
    pub address: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    #[schema(example = "RERA/TN/2025/000456")]
    pub rera_id: Option<String>,
    #[schema(example = 240)]
    pub total_units: Option<i32>,
    #[schema(example = 3)]
    pub total_towers: Option<i32>,
    pub unit_configurations: Option<Vec<String>>,
    pub price_range_min: Option<i64>,
    pub price_range_max: Option<i64>,
    pub area_range_min_sqft: Option<i32>,
    pub area_range_max_sqft: Option<i32>,
    pub possession_date: Option<chrono::NaiveDate>,
    pub launch_date: Option<chrono::NaiveDate>,
    pub amenities: Option<Vec<String>>,
    pub nearby_places: Option<serde_json::Value>,
    pub images: Option<serde_json::Value>,
    pub brochure_url: Option<String>,
    pub video_url: Option<String>,
    pub master_plan_image_url: Option<String>,
    pub floor_plans: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateProjectRequest {
    pub project_name: Option<String>,
    pub project_type: Option<String>,
    pub status: Option<String>,
    pub description: Option<String>,
    pub city: Option<String>,
    pub locality: Option<String>,
    pub address: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub rera_id: Option<String>,
    pub total_units: Option<i32>,
    pub total_towers: Option<i32>,
    pub unit_configurations: Option<Vec<String>>,
    pub price_range_min: Option<i64>,
    pub price_range_max: Option<i64>,
    pub area_range_min_sqft: Option<i32>,
    pub area_range_max_sqft: Option<i32>,
    pub possession_date: Option<chrono::NaiveDate>,
    pub launch_date: Option<chrono::NaiveDate>,
    pub amenities: Option<Vec<String>>,
    pub nearby_places: Option<serde_json::Value>,
    pub images: Option<serde_json::Value>,
    pub brochure_url: Option<String>,
    pub video_url: Option<String>,
    pub master_plan_image_url: Option<String>,
    pub floor_plans: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[allow(dead_code)]
pub struct ProjectLead {
    pub id: Uuid,
    pub project_id: Uuid,
    pub property_id: Option<Uuid>,
    pub name: String,
    pub phone: String,
    pub message: Option<String>,
    pub preferred_visit_date: Option<chrono::NaiveDate>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateProjectLeadRequest {
    pub name: String,
    pub phone: String,
    pub message: Option<String>,
    pub preferred_visit_date: Option<chrono::NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[allow(dead_code)]
pub struct AttachUnitRequest {
    pub property_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BuilderProjectWithStats {
    #[serde(flatten)]
    pub project: BuilderProject,
    pub units_sold: i64,
    pub visits_count: i64,
    pub leads_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProjectBuilderInfo {
    pub id: Uuid,
    pub name: String,
    pub logo: Option<String>,
    pub is_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProjectReviewSummary {
    pub average_rating: f64,
    pub total_reviews: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProjectDetailResponse {
    #[serde(flatten)]
    pub project: BuilderProject,
    pub builder_info: ProjectBuilderInfo,
    pub review_summary: ProjectReviewSummary,
    pub related_units: Vec<serde_json::Value>, // Using generic JSON for related units
}
