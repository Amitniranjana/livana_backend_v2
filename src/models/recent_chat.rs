// src/models/recent_chat.rs
//
// Data model returned by the GET /api/v1/chats/recent endpoint.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

/// One entry in the recent-chats list.
/// Mapped directly from the SQLx query result.
#[derive(Debug, Serialize, FromRow)]
pub struct RecentChat {
    /// UUID of the chat thread.
    pub chat_id: Uuid,
    /// Content of the most recent message in this thread.
    pub last_message: String,
    /// Type of the most recent message: "text" | "image" | "document"
    pub message_type: String,
    /// UTC timestamp of the most recent message.
    pub last_message_time: DateTime<Utc>,
    /// UUID of the other participant in this chat.
    pub other_user_id: Uuid,
    /// Display name of the other participant.
    pub other_user_name: String,
    /// Profile image URL of the other participant (nullable).
    pub other_user_image: Option<String>,
}
