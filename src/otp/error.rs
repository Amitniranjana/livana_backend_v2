// src/otp/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OtpError {
    #[error("AWS error: {0}")]
    AwsError(String),

    #[error("Invalid destination: {0}")]
    InvalidDestination(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
