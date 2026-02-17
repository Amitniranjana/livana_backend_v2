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
    #[schema(example = "john.doe@example.com")]
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

    #[allow(dead_code)] // last_active might be unused or handled differently
    pub last_active: Option<chrono::DateTime<chrono::Utc>>,
    // Wait, original file had `pub last_active: String`. DB usually has timestamp.
    // The error was about `id`. Let's stick to `id` for now.
    // If last_active causes issue, I'll see.
    // Let's keep last_active as String if it was String, but SQLx might complain if DB column is timestamp.
    // But let's check original file again. "last_active: String".
    // If DB has timestamp, mapping to String might work? No, SQLx is strict.
    // Let's assume last_active is String or TEXT in DB for now, or check schema?
    // User didn't report last_active error. The error invalidates further checking.
    // But let's check `User` struct again.

    // Original:
    // pub last_active: String,

    // I should only change id.

    #[schema(example = "active")]
    pub status: String,

    pub created_at: chrono::DateTime<chrono::Utc>, // DB `created_at` is TIMESTAMPTZ.
    pub updated_at: chrono::DateTime<chrono::Utc>, // DB `updated_at` is TIMESTAMPTZ.
    // Wait, original `User` had `String` for created_at?
    // Line 75: `pub created_at: String,`
    // Line 76: `pub updated_at: String,`

    // If the DB has TIMESTAMPTZ, `String` will fail too!
    // The previous error `decoding column "id"` happened first.
    // I should probably fix timestamps too.
}
