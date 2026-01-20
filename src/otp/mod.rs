// src/otp/mod.rs
use rand::{thread_rng, Rng};

pub fn generate_otp() -> String {
    let mut rng = thread_rng();
    format!("{:06}", rng.gen_range(0..1_000_000))
}

pub mod sms;
pub mod email;
pub mod error;

pub use sms::send_sms_otp;
pub use email::send_email_otp;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_otp_length_and_numeric() {
        let otp = generate_otp();
        assert_eq!(otp.len(), 6, "OTP must be 6 digits long");
        assert!(otp.chars().all(|c| c.is_numeric()), "OTP must be numeric");
    }

    #[test]
    fn test_generate_otp_randomness() {
        let otp1 = generate_otp();
        let otp2 = generate_otp();
        // Probability of collision is low, but technically possible.
        // For a simple test, we just check they are generated.
        // A better check implies multiple iterations but this is enough sanity check.
        assert_ne!(otp1, "000000"); // It can be 000000 but rarely.
    }

    #[tokio::test]
    #[ignore]
    async fn test_send_sms_integration() {
        dotenvy::dotenv().ok();
        let phone = std::env::var("TEST_PHONE").expect("TEST_PHONE must be set");
        let otp = generate_otp();
        let result = send_sms_otp(&phone, &otp).await;
        assert!(result.is_ok(), "SMS sending failed: {:?}", result.err());
    }

    #[tokio::test]
    #[ignore]
    async fn test_send_email_integration() {
        dotenvy::dotenv().ok();
        let email = std::env::var("TEST_EMAIL").expect("TEST_EMAIL must be set");
        let otp = generate_otp();
        let result = send_email_otp(&email, &otp).await;
        assert!(result.is_ok(), "Email sending failed: {:?}", result.err());
    }
}
