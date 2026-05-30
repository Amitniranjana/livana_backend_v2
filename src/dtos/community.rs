use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Request body for creating a community.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CreateCommunityDto {
    pub name: String,
    pub description: Option<String>,
}

/// Response after creating a community.
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct CommunityResponseDto {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub is_joined: bool,
}

/// Request body for posting in a community.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CreateCommunityPostDto {
    pub content: String,
    pub images: Option<serde_json::Value>,
}

/// Response after creating a community post.
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct CommunityPostResponseDto {
    pub post_id: Uuid,
    pub community_id: Uuid,
    pub author_id: Uuid,
    pub content: String,
    pub images: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Partial update for a community — only provided fields are updated.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct UpdateCommunityDto {
    pub name: Option<String>,
    pub description: Option<String>,
}

/// Update a community post — content is REQUIRED and cannot be empty.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct UpdateCommunityPostDto {
    pub content: String,
}
