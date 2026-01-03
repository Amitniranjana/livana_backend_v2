// src/otp/sms.rs
use aws_sdk_sns::Client;
use crate::otp::error::OtpError;

pub async fn send_sms_otp(phone: &str, otp: &str) -> Result<(), OtpError> {
    if !phone.starts_with('+') {
        return Err(OtpError::InvalidDestination(
            "Phone must be in E.164 format".into(),
        ));
    }

    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = Client::new(&config);

    let message = format!("Your OTP is {}. Do not share it.", otp);

    let resp = client
        .publish()
        .phone_number(phone)
        .message(message)
        .send()
        .await
        .map_err(|e| OtpError::AwsError(e.to_string()))?;

    if let Some(id) = resp.message_id() {
        tracing::info!("SNS OTP sent, message_id={}", id);
    }

    Ok(())
}
