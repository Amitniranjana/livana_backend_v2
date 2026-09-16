use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, Type, Clone, PartialEq)]
#[sqlx(type_name = "nbfc_status", rename_all = "snake_case")]
pub enum NbfcStatus {
    Active,
    Inactive,
    Suspended,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct NbfcPartner {
    pub id: Uuid,
    pub name: String,
    pub entity_type: String,
    pub revenue_share_pct: f64,
    pub fldg_buffer_pct: f64,
    pub status: NbfcStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
