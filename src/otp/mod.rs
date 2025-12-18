use rand::{thread_rng, Rng};

pub fn generate_otp() -> String {
    let mut rng = thread_rng();
    format!("{:06}", rng.gen_range(0..1_000_000))
}

mod sms;
mod email;
mod error;

pub use sms::send_sms_otp;
pub use email::send_email_otp;
pub use error::OtpError;