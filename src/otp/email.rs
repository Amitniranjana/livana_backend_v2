// src/otp/email.rs
use aws_sdk_sesv2::{Client, types::{Content, Body, Message, EmailContent, Destination}};
use crate::otp::error::OtpError;
use std::env;

pub async fn send_email_otp(email: &str, otp: &str) -> Result<(), OtpError> {
    let from = env::var("SES_FROM_EMAIL")
        .map_err(|_| OtpError::Internal("SES_FROM_EMAIL not set".into()))?;

    let config = aws_config::load_from_env().await;
    let client = Client::new(&config);

    // FIX: Added .unwrap() because build() returns a Result
    let subject = Content::builder()
        .data("Your OTP Code")
        .charset("UTF-8")
        .build()
        .map_err(|e| OtpError::Internal(format!("Failed to build subject: {}", e)))?;

    // FIX: Added .unwrap() or error handling here too
    let body = Content::builder()
        .data(format!("Your OTP is {}. It will expire soon.", otp))
        .charset("UTF-8")
        .build()
        .map_err(|e| OtpError::Internal(format!("Failed to build body: {}", e)))?;

    let message = Message::builder()
        .subject(subject) // Ab ye 'Content' type hai, Result nahi
        .body(Body::builder().text(body).build())
        .build();

    client
        .send_email()
        .from_email_address(from)
        .destination(Destination::builder().to_addresses(email).build())
        .content(EmailContent::builder().simple(message).build())
        .send()
        .await
        .map_err(|e| OtpError::AwsError(e.to_string()))?;

    tracing::info!("SES OTP email sent to {}", email);
    Ok(())
}