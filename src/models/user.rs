use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[allow(dead_code)]
pub struct UserProfile {
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub user_id: String, // foreign key to User

    #[schema(example = "male")]
    pub gender: String,

    #[schema(example = "https://example.com/profile.jpg")]
    pub profile_image_url: Option<String>,

    #[schema(example = "Software developer with 5 years of experience")]
    pub bio: Option<String>,

    // Broker-specific fields
    #[schema(example = "Premium Properties")]
    pub business_name: Option<String>,

    #[schema(example = "BRK123456")]
    pub license_number: Option<String>,

    #[schema(example = 5)]
    pub experience_years: Option<i32>,

    #[schema(example = 2.5)]
    pub commission_rate: Option<f64>,

    pub service_areas: Option<serde_json::Value>,

    #[schema(example = 4.5)]
    pub broker_rating: Option<f64>,

    #[schema(example = 25)]
    pub total_reviews: Option<i32>,

    pub verification_documents: Option<serde_json::Value>,
    pub is_verified_broker: bool,
}


#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[allow(dead_code)]
pub struct User {
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub id: uuid::Uuid,

    #[schema(example = "John")]
    pub first_name: String,

    #[schema(example = "1234567890")]
    pub phone_no: String,

    #[schema(example = "Doe")]
    pub last_name: String,
    pub email: String,
    pub chime_user_arn: Option<String>,
    pub password: String,

    #[schema(example = "user")]
    pub user_role: String,

    pub verified: bool,

    pub last_active: Option<chrono::DateTime<chrono::Utc>>,

    #[schema(example = "active")]
    pub status: String,

    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,

    // Google OAuth fields (nullable — only set for Google-authenticated users)
    /// Stable Google subject ID (`sub` from tokeninfo). NULL for email/password users.
    pub google_id: Option<String>,
    /// Profile picture URL from Google. NULL for email/password users.
    pub profile_picture: Option<String>,
}
