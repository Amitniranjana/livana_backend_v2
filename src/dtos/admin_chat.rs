use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

// Issue 32
#[derive(Debug, Deserialize)]
pub struct SendAdminMessageRequest {
    pub thread_id: Option<Uuid>,
    pub message: String,
    pub attachment_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AdminChatMessageResponse {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub sender_id: String,
    pub sender_role: String,
    pub message: String,
    pub attachment_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

// Issue 33
#[derive(Debug, Deserialize)]
pub struct AdminChatMessagesQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub since: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct AdminChatMessagesResponse {
    pub success: bool,
    pub data: AdminChatMessagesData,
}

#[derive(Debug, Serialize)]
pub struct AdminChatMessagesData {
    pub messages: Vec<AdminChatMessageResponse>,
    pub total: i64,
}

// Issue 33b
#[derive(Debug, Deserialize)]
pub struct AdminChatThreadsQuery {
    pub status: Option<String>, // "open", "closed"
    pub user_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct AdminChatThreadResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub admin_id: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_message: Option<String>, // Last message preview
}

#[derive(Debug, Serialize)]
pub struct AdminChatThreadsListResponse {
    pub success: bool,
    pub data: Vec<AdminChatThreadResponse>,
}
