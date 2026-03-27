// src/otp/sms.rs
use crate::otp::error::OtpError;
use aws_sdk_sns::Client;

pub async fn send_sms_otp(phone: &str, otp: &str) -> Result<(), OtpError> {
    if !phone.starts_with('+') || phone.len() < 7 {
        return Err(OtpError::InvalidDestination(
            "Phone must be in E.164 format (e.g., +1234567890)".into(),
        ));
    }

    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = Client::new(&config);

    let message = format!("Your OTP is {}. Do not share it.", otp);

    let mut publish_builder = client.publish().phone_number(phone).message(message);

    if let Ok(sender_id) = std::env::var("AWS_SNS_SENDER_ID") {
        let attr = aws_sdk_sns::types::MessageAttributeValue::builder()
            .data_type("String")
            .string_value(sender_id)
            .build();
        // In some SDK versions this returns a Result, in others the struct.
        // If it returns Result, we need to handle it.
        // Based on previous error "expected struct ... found enum Result", it returns Result.

        // Use map_err or unwrap. Since this is optional/config, unwrap is okay if we are sure it builds,
        // but better to be safe. But wait, if I use `?` inside `if let`, I return from function.
        // It's fine.
        let attr =
            attr.map_err(|e| OtpError::Internal(format!("Failed to build SenderID: {}", e)))?;

        publish_builder = publish_builder.message_attributes("AWS.SNS.SMS.SenderID", attr);
    }

    let resp = publish_builder
        .send()
        .await
        .map_err(|e| OtpError::AwsError(e.to_string()))?;

    if let Some(id) = resp.message_id() {
        tracing::info!("SNS OTP sent to {}, message_id={}", phone, id);
    }

    Ok(())
}
