use serde::Serialize;
use uuid::Uuid;

/// Response DTO for save / unsave property actions.
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct SavedPropertyResponseDto {
    pub property_id: Uuid,
    pub is_saved: bool,
}

/// Simplified property DTO returned when listing saved properties.
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct PropertyDto {
    pub id: Uuid,
    pub title: String,
    pub price: Option<i64>,
    pub location: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
