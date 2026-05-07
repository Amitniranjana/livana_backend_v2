use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub user_id: String, // Can be used to map to internal DB user ID
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateChannelRequest {
    pub name: String,
    pub mode: String,                 // e.g., "RESTRICTED", "UNRESTRICTED"
    pub privacy: String,              // e.g., "PRIVATE", "PUBLIC"
    pub participant_ids: Vec<String>, // Initial members (User UUIDs)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddMemberRequest {
    pub member_arn: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatUser {
    pub app_instance_user_arn: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatChannel {
    pub channel_arn: String,
    pub name: String,
    pub mode: String,
    pub privacy: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub channel_arn: String,
    pub content: String,
    pub metadata: Option<String>,
}
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatAuthResponse {
    pub app_instance_user_arn: String,
    pub creds: Option<String>, // Placeholder if we return temporary creds
}

use chrono::{DateTime, Utc};
use uuid::Uuid;

// Database se raw row fetch karne ke liye
#[derive(Debug, sqlx::FromRow)]
pub struct ChatRow {
    pub chat_id: Uuid,
    pub participant_id: Uuid,
    pub participant_name: String,
    pub participant_image: Option<String>,
    pub last_message_text: Option<String>,
    pub last_message_time: Option<DateTime<Utc>>,
    pub unread_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    #[serde(rename = "new_message")]
    NewMessage {
        message_id: Uuid,
        chat_id: Uuid,
        sender_id: Uuid,
        content: String,
        message_type: String,
        created_at: DateTime<Utc>,
    },
    #[serde(rename = "message_delivered")]
    MessageDelivered {
        message_id: Uuid,
        delivered_at: DateTime<Utc>,
    },
    #[serde(rename = "message_seen")]
    MessageSeen {
        conversation_id: Uuid,
        seen_by: Uuid,
        seen_at: DateTime<Utc>,
    },
    #[serde(rename = "notification")]
    Notification {
        title: String,
        body: String,
        conversation_id: Uuid,
    },
}

// Final JSON response structs
#[derive(Debug, Serialize)]
pub struct ParticipantInfo {
    pub id: Uuid,
    pub name: String,
    pub profile_image: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LastMessage {
    pub text: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ChatItem {
    pub chat_id: Uuid,
    pub participant: ParticipantInfo,
    pub last_message: Option<LastMessage>,
    pub unread_count: i32,
}

#[derive(Debug, Serialize)]
pub struct ChatListResponse {
    pub success: bool,
    pub data: Vec<ChatItem>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub message: String,
    pub error_code: String,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ChatExistsRow {
    #[allow(dead_code)]
    pub chat_id: Uuid,
    pub is_deleted: bool,
}
