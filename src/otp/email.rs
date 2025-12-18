use tracing::info;

use crate::otp::OtpError;

pub async fn send_email_otp(_email: &str, otp: &str) -> Result<(), OtpError> {
    // Lightweight fallback implementation: log OTP instead of sending through AWS SES.
    // Replace with a real provider (AWS SES) and remove this stub when ready.
    info!("(stub) Email OTP would be sent: {}", otp);
    Ok(())
}