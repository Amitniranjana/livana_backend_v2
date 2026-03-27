use crate::models::chat::{ChatChannel, ChatUser};
use anyhow::{Context, Result};
use aws_config::SdkConfig;
use aws_sdk_chimesdkidentity::Client as IdentityClient;
use aws_sdk_chimesdkmessaging::Client as MessagingClient;

#[derive(Clone)]
pub struct ChatService {
    identity_client: IdentityClient,
    messaging_client: MessagingClient,
    #[allow(dead_code)]
    app_instance_arn: String, // We might need this, or we can look it up/pass it
}

impl ChatService {
    pub fn new(config: &SdkConfig, app_instance_arn: String) -> Self {
        let identity_client = IdentityClient::new(config);
        let messaging_client = MessagingClient::new(config);
        Self {
            identity_client,
            messaging_client,
            app_instance_arn,
        }
    }

    /// Create an App Instance User in Chime
    pub async fn create_app_instance_user(
        &self,
        app_instance_arn: &str,
        user_id: &str,
        name: &str,
    ) -> Result<ChatUser> {
        let resp = self
            .identity_client
            .create_app_instance_user()
            .app_instance_arn(app_instance_arn)
            .app_instance_user_id(user_id)
            .name(name)
            .send()
            .await
            .context("Failed to create app instance user")?;

        let user_arn = resp.app_instance_user_arn().unwrap_or_default().to_string();

        Ok(ChatUser {
            app_instance_user_arn: user_arn,
            name: name.to_string(),
        })
    }

    /// Create a Chat Channel
    pub async fn create_channel(
        &self,
        app_instance_arn: &str,
        name: &str,
        mode: &str,
        privacy: &str,
        cleaner_arn: &str,
    ) -> Result<ChatChannel> {
        // Mode: RESTRICTED or UNRESTRICTED
        // Privacy: PRIVATE or PUBLIC
        let mode_enum = aws_sdk_chimesdkmessaging::types::ChannelMode::from(mode);
        let privacy_enum = aws_sdk_chimesdkmessaging::types::ChannelPrivacy::from(privacy);

        let resp = self
            .messaging_client
            .create_channel()
            .app_instance_arn(app_instance_arn)
            .name(name)
            .mode(mode_enum)
            .privacy(privacy_enum)
            .chime_bearer(cleaner_arn) // The user creating the channel
            .send()
            .await
            .context("Failed to create channel")?;

        let channel_arn = resp.channel_arn().unwrap_or_default().to_string();

        Ok(ChatChannel {
            channel_arn,
            name: name.to_string(),
            mode: mode.to_string(),
            privacy: privacy.to_string(),
        })
    }

    /// Add a Member to a Channel
    pub async fn add_channel_flow(
        &self,
        channel_arn: &str,
        member_arn: &str,
        chimer_bearer: &str,
    ) -> Result<()> {
        self.messaging_client
            .create_channel_membership()
            .channel_arn(channel_arn)
            .member_arn(member_arn)
            .r#type(aws_sdk_chimesdkmessaging::types::ChannelMembershipType::Default)
            .chime_bearer(chimer_bearer) // Admin acting on behalf
            .send()
            .await
            .context("Failed to add member to channel")?;
        Ok(())
    }

    /// Send a message (Control plane send)
    pub async fn send_message(
        &self,
        channel_arn: &str,
        content: &str,
        sender_arn: &str,
    ) -> Result<String> {
        let resp = self
            .messaging_client
            .send_channel_message()
            .channel_arn(channel_arn)
            .content(content)
            .r#type(aws_sdk_chimesdkmessaging::types::ChannelMessageType::Standard)
            .persistence(
                aws_sdk_chimesdkmessaging::types::ChannelMessagePersistenceType::Persistent,
            )
            .chime_bearer(sender_arn)
            .send()
            .await
            .context("Failed to send message")?;

        Ok(resp.message_id().unwrap_or_default().to_string())
    }

    // Helper to get endpoint config for WebSocket if needed (usually handled by DescribeAppInstanceUserEndpoint)
    pub async fn get_messaging_endpoint(&self) -> Result<String> {
        let resp = self
            .messaging_client
            .get_messaging_session_endpoint()
            .send()
            .await
            .context("Failed to get messaging session endpoint")?;

        let endpoint = resp.endpoint().context("No endpoint in response")?;
        let url = endpoint.url().context("No URL in endpoint")?;
        Ok(url.to_string())
    }
}
