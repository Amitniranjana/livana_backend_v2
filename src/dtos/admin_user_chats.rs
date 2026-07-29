use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AdminUserChatsQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub search: Option<String>,
    pub is_blocked: Option<bool>,
    pub is_archived: Option<bool>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AdminUserChatParticipant {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub profile_picture: Option<String>,
    pub joined_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AdminUserChatResponse {
    pub id: Uuid,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub participants: Vec<AdminUserChatParticipant>,
    pub last_message: Option<String>,
    pub last_message_at: Option<DateTime<Utc>>,
    pub is_blocked: bool,
    pub is_archived: bool,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AdminUserChatsListResponse {
    pub success: bool,
    pub data: AdminUserChatsListData,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AdminUserChatsListData {
    pub total: i64,
    pub chats: Vec<AdminUserChatResponse>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AdminUserChatMessagesQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AdminUserChatMessageResponse {
    pub id: Uuid,
    pub chat_id: Uuid,
    pub sender_id: Option<Uuid>,
    pub sender_name: Option<String>,
    pub sender_email: Option<String>,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AdminUserChatMessagesListResponse {
    pub success: bool,
    pub data: AdminUserChatMessagesListData,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AdminUserChatMessagesListData {
    pub total: i64,
    pub messages: Vec<AdminUserChatMessageResponse>,
}
