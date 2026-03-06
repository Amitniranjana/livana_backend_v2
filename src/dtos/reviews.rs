use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[allow(dead_code)]
#[derive(Debug, Deserialize, Validate)]
pub struct CreateReviewDto {
    pub associate_id: Uuid,
    #[validate(range(min = 1, max = 5, message = "Rating must be between 1 and 5"))]
    pub rating: u8,
    pub comment: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct CreateReviewResponseDto {
    pub review_id: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ReviewDto {
    pub review_id: Uuid,
    pub user_id: Uuid,
    pub rating: u8,
    pub comment: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
