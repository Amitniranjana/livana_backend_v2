use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Requests ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateCarecrewReviewRequest {
    pub booking_id: String,
    pub provider_id: String,
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
pub struct ReviewsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ── Responses ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct CreateReviewData {
    pub review_id: Uuid,
    pub booking_id: Uuid,
    pub provider_id: Uuid,
    pub reviewer_id: Uuid,
    pub rating: f64,
    pub comment: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ReviewItem {
    pub id: Uuid,
    pub reviewer_name: String,
    pub reviewer_image: Option<String>,
    pub rating: f64,
    pub comment: Option<String>,
    pub reply: Option<String>,
    pub reply_at: Option<DateTime<Utc>>,
    pub review_date: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ReviewBreakdown {
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
pub struct ReviewSummary {
    pub average_rating: f64,
    pub total_reviews: i64,
    pub breakdown: ReviewBreakdown,
}

#[derive(Debug, Serialize)]
pub struct ReviewsListData {
    pub reviews: Vec<ReviewItem>,
    pub summary: ReviewSummary,
    pub total_count: i64,
    pub current_page: i64,
    pub total_pages: i64,
}

#[derive(Debug, Serialize)]
pub struct EditReviewData {
    pub review_id: Uuid,
    pub rating: f64,
    pub comment: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct DeleteReviewData {
    pub deleted: bool,
    pub review_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct ReplyData {
    pub review_id: Uuid,
    pub reply: String,
    pub replied_at: DateTime<Utc>,
}
