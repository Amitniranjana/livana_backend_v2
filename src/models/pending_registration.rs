use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[allow(dead_code)]
pub struct PendingRegistration {
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub password_hash: String,
    pub phone_no: String,
    pub gender: String,
    pub user_role: String,
    pub business_name: Option<String>,
    pub license_number: Option<String>,
    pub experience_years: Option<i32>,
    pub commission_rate: Option<f64>,
    pub ref_code: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
