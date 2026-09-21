use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Type, Clone, PartialEq)]
#[sqlx(type_name = "nbfc_user_role", rename_all = "snake_case")]
pub enum NbfcUserRole {
    Admin,
    Underwriter,
}

#[derive(Debug, Serialize, Deserialize, Type, Clone, PartialEq)]
#[sqlx(type_name = "nbfc_user_status", rename_all = "snake_case")]
pub enum NbfcUserStatus {
    Active,
    Inactive,
    Suspended,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct NbfcUser {
    pub id: Uuid,
    pub nbfc_id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub role: NbfcUserRole,
    pub status: NbfcUserStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NbfcClaims {
    pub sub: String, // NbfcUser id
    pub nbfc_id: Uuid,
    pub exp: usize,
}
