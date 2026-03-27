use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Request DTOs ──

#[derive(Debug, Deserialize)]
pub struct CreatePropertyReviewRequest {
    pub visit_id: Uuid,
    pub property_id: Uuid,
    pub rating: f64,
    pub comment: Option<String>,
    pub location_rating: Option<f64>,
    pub cleanliness_rating: Option<f64>,
    pub value_rating: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct EditPropertyReviewRequest {
    pub rating: Option<f64>,
    pub comment: Option<String>,
    pub location_rating: Option<f64>,
    pub cleanliness_rating: Option<f64>,
    pub value_rating: Option<f64>,
}

// ── Response DTOs ──

#[derive(Debug, Serialize)]
pub struct PropertyReviewCreatedData {
    pub review_id: Uuid,
    pub visit_id: Uuid,
    pub property_id: Uuid,
    pub reviewer_id: Uuid,
    pub rating: f64,
    pub comment: Option<String>,
    pub location_rating: Option<f64>,
    pub cleanliness_rating: Option<f64>,
    pub value_rating: Option<f64>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct PropertyReviewUpdatedData {
    pub review_id: Uuid,
    pub rating: f64,
    pub comment: Option<String>,
    pub location_rating: Option<f64>,
    pub cleanliness_rating: Option<f64>,
    pub value_rating: Option<f64>,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct PropertyReviewItem {
    pub id: Uuid,
    pub reviewer_name: String,
    pub reviewer_image: Option<String>,
    pub rating: f64,
    pub comment: Option<String>,
    pub location_rating: Option<f64>,
    pub cleanliness_rating: Option<f64>,
    pub value_rating: Option<f64>,
    pub reply: Option<String>,
    pub review_date: String,
}

#[derive(Debug, Serialize)]
pub struct PropertyReviewSummary {
    pub average_rating: f64,
    pub total_reviews: i64,
    pub average_location_rating: Option<f64>,
    pub average_cleanliness_rating: Option<f64>,
    pub average_value_rating: Option<f64>,
    pub breakdown: crate::dtos::carecrew_review::RatingBreakdown,
}

#[derive(Debug, Serialize)]
pub struct PropertyReviewsListData {
    pub reviews: Vec<PropertyReviewItem>,
    pub summary: PropertyReviewSummary,
    pub total_count: i64,
    pub current_page: i64,
    pub total_pages: i64,
}
