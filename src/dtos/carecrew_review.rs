use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Request DTOs ──

#[derive(Debug, Deserialize)]
pub struct CreateCarecrewReviewRequest {
    pub booking_id: Uuid,
    pub provider_id: Uuid,
    pub rating: f64,
    pub comment: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EditCarecrewReviewRequest {
    pub rating: Option<f64>,
    pub comment: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReplyRequest {
    pub reply: String,
}

#[derive(Debug, Deserialize)]
pub struct ReviewListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ── Response DTOs ──

#[derive(Debug, Serialize)]
pub struct CarecrewReviewCreatedData {
    pub review_id: Uuid,
    pub booking_id: Uuid,
    pub provider_id: Uuid,
    pub reviewer_id: Uuid,
    pub rating: f64,
    pub comment: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct CarecrewReviewUpdatedData {
    pub review_id: Uuid,
    pub rating: f64,
    pub comment: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct ReviewDeletedData {
    pub deleted: bool,
    pub review_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct ReviewReplyData {
    pub review_id: Uuid,
    pub reply: String,
    pub replied_at: String,
}

#[derive(Debug, Serialize)]
pub struct CarecrewReviewItem {
    pub id: Uuid,
    pub reviewer_name: String,
    pub reviewer_image: Option<String>,
    pub rating: f64,
    pub comment: Option<String>,
    pub reply: Option<String>,
    pub reply_at: Option<String>,
    pub review_date: String,
}

#[derive(Debug, Serialize)]
pub struct RatingBreakdown {
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
pub struct CarecrewReviewSummary {
    pub average_rating: f64,
    pub total_reviews: i64,
    pub breakdown: RatingBreakdown,
}

#[derive(Debug, Serialize)]
pub struct CarecrewReviewsListData {
    pub reviews: Vec<CarecrewReviewItem>,
    pub summary: CarecrewReviewSummary,
    pub total_count: i64,
    pub current_page: i64,
    pub total_pages: i64,
}
