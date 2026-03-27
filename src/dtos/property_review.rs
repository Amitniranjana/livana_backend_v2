use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Requests ─────────────────────────────────────────────────────────────────

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

// ── Responses ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct CreatePropertyReviewData {
    pub review_id: Uuid,
    pub visit_id: Uuid,
    pub property_id: Uuid,
    pub reviewer_id: Uuid,
    pub rating: f64,
    pub comment: Option<String>,
    pub location_rating: Option<f64>,
    pub cleanliness_rating: Option<f64>,
    pub value_rating: Option<f64>,
    pub created_at: DateTime<Utc>,
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
    pub reply_at: Option<DateTime<Utc>>,
    pub review_date: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PropertyReviewBreakdown {
    #[serde(rename = "5")]
    pub five: i64,
    #[serde(rename = "4")]
    pub four: i64,
    #[serde(rename = "3")]
    pub three: i64,
    #[serde(rename = "2")]
    pub two: i64,
    #[serde(rename = "1")]
    pub one: i64,
}

#[derive(Debug, Serialize)]
pub struct PropertyReviewSummary {
    pub average_rating: f64,
    pub total_reviews: i64,
    pub average_location_rating: Option<f64>,
    pub average_cleanliness_rating: Option<f64>,
    pub average_value_rating: Option<f64>,
    pub breakdown: PropertyReviewBreakdown,
}

#[derive(Debug, Serialize)]
pub struct PropertyReviewsListData {
    pub reviews: Vec<PropertyReviewItem>,
    pub summary: PropertyReviewSummary,
    pub total_count: i64,
    pub current_page: i64,
    pub total_pages: i64,
}

#[derive(Debug, Serialize)]
pub struct EditPropertyReviewData {
    pub review_id: Uuid,
    pub rating: f64,
    pub comment: Option<String>,
    pub location_rating: Option<f64>,
    pub cleanliness_rating: Option<f64>,
    pub value_rating: Option<f64>,
    pub updated_at: DateTime<Utc>,
}
