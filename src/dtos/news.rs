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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewsUpdateRequest {
    pub headline: Option<String>,
    pub short_summary: Option<String>,
    pub source: Option<String>,
    pub category: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub thumbnail_url: Option<String>,
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
}
