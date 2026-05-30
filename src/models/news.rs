use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct NewsItem {
    pub id: Uuid,
    pub headline: String,
    pub short_summary: String,
    pub source: Option<String>,
    pub category: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub thumbnail_url: Option<String>,
    pub views: i32,
    pub clicks: i32,
    pub shares: i32,
    pub engagement_velocity: f64,
    pub is_trending: bool,
    pub force_trending: bool,
    pub notifications_disabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub author_id: Option<Uuid>,
    pub status: String,
    pub images: Option<serde_json::Value>,
}
