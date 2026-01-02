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
