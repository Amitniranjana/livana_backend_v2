// src/otp/email.rs
use crate::otp::error::OtpError;
use aws_sdk_sesv2::{
    Client,
    types::{Body, Content, Destination, EmailContent, Message},
};
use std::env;

pub async fn send_email_otp(email: &str, otp: &str) -> Result<(), OtpError> {
    let from = env::var("SES_FROM_EMAIL")
        .map_err(|_| OtpError::Internal("SES_FROM_EMAIL environment variable not set".into()))?;

    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = Client::new(&config);

    let subject = Content::builder()
        .data("Your OTP Code")
        .charset("UTF-8")
        .build()
        // If build returns generic result
        .map_err(|_| OtpError::Internal("Failed to build email subject".into()))?;

    let body_text = format!("Your OTP is {}. It will expire soon.", otp);
    let body_content = Content::builder()
        .data(body_text)
        .charset("UTF-8")
        .build()
        .map_err(|_| OtpError::Internal("Failed to build email body content".into()))?;

    let body = Body::builder().text(body_content).build(); // Body builder usually returns Body

    let message = Message::builder().subject(subject).body(body).build(); // Message builder returns Message

    let dest = Destination::builder().to_addresses(email).build();

    let email_content = EmailContent::builder().simple(message).build();

    let output = client
        .send_email()
        .from_email_address(from)
        .destination(dest)
        .content(email_content)
        .send()
        .await
        .map_err(|e| OtpError::AwsError(e.to_string()))?;

    if let Some(id) = output.message_id() {
        tracing::info!("SES OTP email sent to {}, message_id={}", email, id);
    } else {
        tracing::info!("SES OTP email sent to {}", email);
    }

    Ok(())
}
