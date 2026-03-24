use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

// ─── Request Bodies ───────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct BookVisitRequest {
    pub property_id: Uuid,
    pub provider_id: Uuid,
    pub scheduled_date_time: DateTime<Utc>,
    pub contact_number: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateVisitStatusRequest {
    pub status: String,
    pub cancellation_reason: Option<String>,
}

// ─── Database Row Structs ─────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
pub struct SiteVisitRow {
    pub visit_id: Uuid,
    pub property_id: Uuid,
    pub property_title: Option<String>,
    pub property_location: Option<String>,
    pub user_id: Uuid,
    pub provider_id: Uuid,
    pub provider_name: Option<String>,
    pub provider_image: Option<String>,
    pub scheduled_date_time: DateTime<Utc>,
    pub status: String,
    pub contact_number: Option<String>,
    pub notes: Option<String>,
    pub cancellation_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ─── Response Structs ─────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct PropertyInfo {
    pub id: Uuid,
    pub title: Option<String>,
    pub location: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProviderInfo {
    pub id: Uuid,
    pub name: Option<String>,
    pub profile_image: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VisitItem {
    pub visit_id: Uuid,
    pub property: PropertyInfo,
    pub provider: ProviderInfo,
    pub scheduled_date_time: DateTime<Utc>,
    pub status: String,
    pub contact_number: Option<String>,
    pub notes: Option<String>,
    pub cancellation_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}
