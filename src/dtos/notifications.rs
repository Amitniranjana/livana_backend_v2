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
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Response DTO for the mark-as-read action.
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct NotificationReadResponseDto {
    pub notification_id: Uuid,
    pub is_read: bool,
}
