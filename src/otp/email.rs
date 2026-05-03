// src/otp/email.rs
//! Email OTP delivery via Gmail SMTP using the `lettre` crate.
//!
//! Reads the following environment variables:
//! - `SMTP_USERNAME`  — Gmail address used for authentication
//! - `SMTP_PASSWORD`  — Gmail App Password (NOT account password)
//! - `SMTP_FROM`      — Sender email address (usually same as username)
//! - `SMTP_FROM_NAME` — Display name shown in the "From" field

use crate::otp::error::OtpError;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, header::ContentType},
    transport::smtp::authentication::Credentials,
};
use std::env;

/// Send an OTP to the given email address via Gmail SMTP.
///
/// `purpose` is a human-readable label shown in the email body,
/// e.g. "Email Verification", "Password Reset", "Change Password".
pub async fn send_email_otp(email: &str, otp: &str, purpose: &str) -> Result<(), OtpError> {
    let smtp_username = env::var("SMTP_USERNAME")
        .map_err(|_| OtpError::Internal("SMTP_USERNAME environment variable not set".into()))?;
    let smtp_password = env::var("SMTP_PASSWORD")
        .map_err(|_| OtpError::Internal("SMTP_PASSWORD environment variable not set".into()))?;
    let smtp_from = env::var("SMTP_FROM")
        .map_err(|_| OtpError::Internal("SMTP_FROM environment variable not set".into()))?;
    let smtp_from_name = env::var("SMTP_FROM_NAME").unwrap_or_else(|_| "Care Connect".into());

    // Build sender and recipient mailboxes
    let from_mailbox: Mailbox = format!("{} <{}>", smtp_from_name, smtp_from)
        .parse()
        .map_err(|e| OtpError::Internal(format!("Invalid FROM address: {}", e)))?;

    let to_mailbox: Mailbox = email
        .parse()
        .map_err(|e| OtpError::InvalidDestination(format!("Invalid recipient email: {}", e)))?;

    // Build HTML body
    let html_body = build_otp_html(otp, purpose);

    let message = Message::builder()
        .from(from_mailbox)
        .to(to_mailbox)
        .subject(format!("Your Care Connect {} OTP", purpose))
        .header(ContentType::TEXT_HTML)
        .body(html_body)
        .map_err(|e| OtpError::Internal(format!("Failed to build email message: {}", e)))?;

    // Build SMTP transport — STARTTLS on port 587
    let creds = Credentials::new(smtp_username, smtp_password);

    let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay("smtp.gmail.com")
        .map_err(|e| OtpError::SmtpError(format!("Failed to create SMTP relay: {}", e)))?
        .credentials(creds)
        .port(587)
        .build();

    // Send with retry (max 1 retry, 2s delay)
    let mut last_err_msg = String::new();
    for attempt in 0..2u8 {
        match mailer.send(message.clone()).await {
            Ok(_) => {
                tracing::info!("SMTP OTP email sent to {} (purpose: {})", email, purpose);
                return Ok(());
            }
            Err(e) => {
                last_err_msg = format!("{}", e);
                tracing::error!(
                    "SMTP send attempt {} failed for {}: {}",
                    attempt + 1,
                    email,
                    last_err_msg
                );
                if attempt == 0 {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
        }
    }

    Err(OtpError::SmtpError(format!(
        "Failed to send OTP email after 2 attempts: {}",
        last_err_msg
    )))
}

/// Build a styled HTML email body containing the OTP.
fn build_otp_html(otp: &str, purpose: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
</head>
<body style="margin:0;padding:0;background-color:#f4f4f7;font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;">
  <table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background-color:#f4f4f7;padding:40px 0;">
    <tr>
      <td align="center">
        <table role="presentation" width="480" cellpadding="0" cellspacing="0"
               style="background:#ffffff;border-radius:12px;box-shadow:0 2px 12px rgba(0,0,0,0.08);overflow:hidden;">
          <!-- Header -->
          <tr>
            <td style="background:linear-gradient(135deg,#4F46E5,#7C3AED);padding:32px 40px;text-align:center;">
              <h1 style="color:#ffffff;margin:0;font-size:24px;font-weight:700;letter-spacing:0.5px;">
                Care Connect
              </h1>
            </td>
          </tr>
          <!-- Body -->
          <tr>
            <td style="padding:40px;">
              <p style="color:#374151;font-size:16px;line-height:1.6;margin:0 0 8px;">
                Hello,
              </p>
              <p style="color:#374151;font-size:16px;line-height:1.6;margin:0 0 24px;">
                Your <strong>{purpose}</strong> verification code is:
              </p>
              <div style="background:#F3F4F6;border-radius:8px;padding:20px;text-align:center;margin:0 0 24px;">
                <span style="font-size:36px;font-weight:700;letter-spacing:8px;color:#4F46E5;">
                  {otp}
                </span>
              </div>
              <p style="color:#6B7280;font-size:14px;line-height:1.5;margin:0 0 8px;">
                This code will expire in <strong>10 minutes</strong>.
              </p>
              <p style="color:#6B7280;font-size:14px;line-height:1.5;margin:0;">
                If you did not request this code, please ignore this email.
              </p>
            </td>
          </tr>
          <!-- Footer -->
          <tr>
            <td style="background:#F9FAFB;padding:20px 40px;text-align:center;border-top:1px solid #E5E7EB;">
              <p style="color:#9CA3AF;font-size:12px;margin:0;">
                &copy; 2026 Care Connect (Livana Eco). All rights reserved.
              </p>
            </td>
          </tr>
        </table>
      </td>
    </tr>
  </table>
</body>
</html>"#,
        purpose = purpose,
        otp = otp,
    )
}
