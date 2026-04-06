use serde::Serialize;
use uuid::Uuid;

/// DTO for a single notification returned to the client.
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct NotificationDto {
    pub id: Uuid,
    pub title: String,
    pub message: String,
    #[serde(rename = "type")]
    pub notification_type: String,
    pub is_read: bool,
    /// UUID of the related entity (vibe_id, visit_id, etc.)
    pub related_entity_id: Option<Uuid>,
    /// Type of the related entity: "VIBE", "SITE_VISIT", etc.
    pub related_entity_type: Option<String>,
    /// Accept/Reject status: null, "ACCEPTED", "REJECTED"
    pub action_status: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Response DTO for the mark-as-read action.
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct NotificationReadResponseDto {
    pub notification_id: Uuid,
    pub is_read: bool,
}
