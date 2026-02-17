use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub user_id: String, // Can be used to map to internal DB user ID
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateChannelRequest {
    pub name: String,
    pub mode: String, // e.g., "RESTRICTED", "UNRESTRICTED"
    pub privacy: String, // e.g., "PRIVATE", "PUBLIC"
    pub user_arns: Vec<String>, // Initial members
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
