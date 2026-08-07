use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Ping {
    pub id: Uuid,
    pub user_id: Uuid,
    pub location: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub property_type: Option<String>,
    pub listing_type: Option<String>,
    pub min_budget: Option<i64>,
    pub max_budget: Option<i64>,
    pub min_bedrooms: Option<i32>,
    pub max_bedrooms: Option<i32>,
    pub note: Option<String>,
    pub status: String,
    pub close_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[allow(dead_code)]
pub struct PingResponse {
    pub id: Uuid,
    pub ping_id: Uuid,
    pub broker_id: Uuid,
    pub chat_id: Uuid,
    pub message: String,
    pub responded_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct PingResponseJoined {
    pub id: Uuid,
    pub broker_id: Uuid,
    pub broker_name: String,
    pub message: String,
    pub chat_id: Uuid,
    pub responded_at: DateTime<Utc>,
}
