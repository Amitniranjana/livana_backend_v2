// src/models/recent_chat.rs
//
// Data model returned by the GET /api/v1/chats/recent endpoint.

use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// One entry in the recent-chats list.
/// Mapped directly from the SQLx query result.
#[derive(Debug, Serialize, FromRow)]
pub struct RecentChat {
    /// UUID of the chat thread.
    pub chat_id: Uuid,
    /// Content of the most recent message in this thread.
    pub last_message: String,
    /// UTC timestamp of the most recent message.
    pub last_message_time: DateTime<Utc>,
}
