use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct NewsCreateRequest {
    pub headline: String,
    pub short_summary: String,
    pub source: Option<String>,
    pub category: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub thumbnail_url: Option<String>,
    pub images: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewsUpdateRequest {
    pub headline: Option<String>,
    pub short_summary: Option<String>,
    pub source: Option<String>,
    pub category: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub thumbnail_url: Option<String>,
    pub images: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewsActionRequest {
    #[serde(default)]
    pub view: bool,
    #[serde(default)]
    pub click: bool,
    #[serde(default)]
    pub share: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AdminNewsActionRequest {
    pub force_trending: Option<bool>,
    pub notifications_disabled: Option<bool>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewsCommentRequest {
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewsReportRequest {
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewsCommentResponse {
    pub id: Uuid,
    pub news_id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
