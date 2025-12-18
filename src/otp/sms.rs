use tracing::info;

use crate::otp::OtpError;
pub async fn send_sms_otp(_phone: &str, otp: &str) -> Result<(), OtpError> {
    // Lightweight fallback implementation: log OTP instead of sending through AWS.
    // Replace with a real provider (AWS SNS) and remove this stub when ready.
    info!("(stub) SMS OTP would be sent: {}", otp);
    Ok(())
}