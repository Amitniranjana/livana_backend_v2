use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── POST /api/expo — Request ────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CreateExpoRequest {
    pub title: String,
    pub description: String,
    pub location: String,
    pub event_date: String,   // "2026-05-10"
    pub start_time: String,   // "10:00"
    pub end_time: String,     // "18:00"
    pub organizer_id: String, // UUID as string
    pub banner_image: Option<String>,
    pub max_participants: i32,
}

// ── POST /api/expo — Response data ──────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct CreateExpoResponseData {
    pub expo_id: Uuid,
    pub title: String,
    pub created_at: String,
}

// ── GET /api/expo — Query params ────────────────────────────────────────────

fn default_limit() -> i64 {
    10
}
fn default_offset() -> i64 {
    0
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ExpoListQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default = "default_offset")]
    pub offset: i64,
    pub city: Option<String>,
}

// ── GET /api/expo — Response data ───────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ExpoEventListItem {
    pub expo_id: Uuid,
    pub title: String,
    pub location: String,
    pub event_date: String,
    pub registered_count: i32,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ExpoEventsData {
    pub events: Vec<ExpoEventListItem>,
}

// ── GET /api/expo/{expo_id} — Response data (API 3) ─────────────────────────

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ExpoDetailData {
    pub expo_id: Uuid,
    pub title: String,
    pub description: String,
    pub location: String,
    pub event_date: String,
    pub start_time: String,
    pub end_time: String,
    pub organizer_id: Uuid,
    pub banner_image: String,
    pub max_participants: i32,
    pub participants_count: i64,
    pub services_available: Vec<String>,
    pub created_at: String,
}

// ── POST /api/expo/{expo_id}/register — Request (API 4) ─────────────────────

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct RegisterExpoRequest {
    pub user_id: String,
    pub user_type: String,
    pub company_name: Option<String>,
}

// ── GET /api/expo/{expo_id}/participants — Query params (API 5) ──────────────

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ExpoParticipantsQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default = "default_offset")]
    pub offset: i64,
    pub user_type: Option<String>,
}

// ── GET /api/expo/{expo_id}/participants — Response data (API 5) ─────────────

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ParticipantItem {
    pub registration_id: Uuid,
    pub user_id: Uuid,
    pub user_type: String,
    pub company_name: Option<String>,
    pub registered_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ExpoParticipantsData {
    pub participants: Vec<ParticipantItem>,
    pub total_count: i64,
    pub current_page: i64,
    pub total_pages: i64,
}
