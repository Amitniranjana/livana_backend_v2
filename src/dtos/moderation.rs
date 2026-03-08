use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Enum for reportable entity types.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntityType {
    User,
    Property,
    Community,
    Post,
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntityType::User => write!(f, "USER"),
            EntityType::Property => write!(f, "PROPERTY"),
            EntityType::Community => write!(f, "COMMUNITY"),
            EntityType::Post => write!(f, "POST"),
        }
    }
}

/// Request body for reporting an entity.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ReportEntityDto {
    pub entity_type: EntityType,
    pub entity_id: Uuid,
    pub reason: String,
}

/// Response after filing a report.
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ReportResponseDto {
    pub report_id: Uuid,
    pub status: String,
}
