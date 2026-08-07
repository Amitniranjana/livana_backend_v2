use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

// Request DTOs
#[derive(Debug, Deserialize)]
pub struct CreatePingRequest {
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
}

#[derive(Debug, Deserialize)]
pub struct ClosePingRequest {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct RespondPingRequest {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct PingQuery {
    pub status: Option<String>, // active, closed, all
}

#[derive(Debug, Deserialize)]
pub struct MatchingPingQuery {
    pub location: Option<String>,
    pub property_type: Option<String>,
    pub listing_type: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// Response DTOs
#[derive(Debug, Serialize)]
pub struct PingDto {
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

#[derive(Debug, Serialize)]
pub struct RespondPingResponseDto {
    pub chat_id: Uuid,
    pub ping_id: Uuid,
    pub broker_id: Uuid,
    pub responded_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PingResponseDto {
    pub id: Uuid,
    pub broker_id: Uuid,
    pub broker_name: String,
    pub message: String,
    pub chat_id: Uuid,
    pub responded_at: DateTime<Utc>,
}
