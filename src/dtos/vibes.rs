use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Request body for sending a vibe.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SendVibeDto {
    pub target_user_id: Uuid,
}

/// Response after sending a vibe.
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct VibeResponseDto {
    pub vibe_id: Uuid,
    pub sender_id: Uuid,
    pub target_user_id: Uuid,
    pub status: String,
}

/// A matched user entry.
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct MatchDto {
    pub match_id: Uuid,
    pub matched_user_id: Uuid,
    pub matched_user_name: String,
    pub matched_user_image: Option<String>,
    pub matched_at: chrono::DateTime<chrono::Utc>,
}
