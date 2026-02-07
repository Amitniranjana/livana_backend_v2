use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[sqlx(type_name = "kyc_doc_type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KycDocType {
    Aadhaar,
    Pan,
    Passport,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::Type, PartialEq)]
#[sqlx(type_name = "kyc_status", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KycStatus {
    Pending,
    Verified,
    Rejected,
    PendingReview,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct KycSubmission {
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub id: Uuid,

    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub user_id: Uuid,

    pub email: String,

    #[schema(example = "Shubham Dixit")]
    pub input_name: String,

    pub doc_type: KycDocType,

    #[serde(skip)]
    pub s3_bucket: String,

    #[serde(skip)]
    pub s3_key: String,

    #[serde(skip)]
    pub file_sha256: String,

    pub extracted_name: Option<String>,

    pub name_match: Option<bool>,

    pub status: KycStatus,

    pub rejection_reason: Option<String>,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,
}
