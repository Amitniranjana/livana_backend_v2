//! Zero Deposit (Credit Tokens) v2 Notification Service
//!
//! Handles all tenant notifications required by Prompt 5:
//! 1. Tokens issued (with expiry date)
//! 2. 7 days and 2 days before expiry
//! 3. New admin message
//! 4. Document requested or rejected
//! 5. Redemption approved
//! 6. Funds released

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn notify_tokens_issued(
    pool: &PgPool,
    user_id: Uuid,
    amount: f64,
    expires_at: DateTime<Utc>,
) {
    let date_str = expires_at.format("%d %b %Y").to_string();
    let title = "Zero Deposit Credit Tokens Issued! 🎉";
    let message = format!(
        "Congratulations! ₹{:.2} in Zero Deposit Credit Tokens have been issued to your wallet. Tokens are valid until {}.",
        amount, date_str
    );

    let _ = sqlx::query(
        "INSERT INTO notifications (id, user_id, title, message, type, is_read, created_at)
         VALUES ($1, $2, $3, $4, 'ZERO_DEPOSIT', false, NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(title)
    .bind(message)
    .execute(pool)
    .await;
}

pub async fn notify_expiry_warning(
    pool: &PgPool,
    user_id: Uuid,
    days_left: i64,
    expires_at: DateTime<Utc>,
) {
    let date_str = expires_at.format("%d %b %Y").to_string();
    let title = format!("Credit Tokens Expiring in {} Days ⏳", days_left);
    let message = format!(
        "Your Zero Deposit Credit Tokens will expire on {}. Initiate your property redemption now to avoid losing your tokens.",
        date_str
    );

    let _ = sqlx::query(
        "INSERT INTO notifications (id, user_id, title, message, type, is_read, created_at)
         VALUES ($1, $2, $3, $4, 'ZERO_DEPOSIT', false, NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(title)
    .bind(message)
    .execute(pool)
    .await;
}

pub async fn notify_document_requested(
    pool: &PgPool,
    user_id: Uuid,
    doc_label: &str,
) {
    let title = "Action Required: Document Requested";
    let message = format!(
        "The Livana team has requested an additional document for your redemption: {}. Please upload it in the chat.",
        doc_label
    );

    let _ = sqlx::query(
        "INSERT INTO notifications (id, user_id, title, message, type, is_read, created_at)
         VALUES ($1, $2, $3, $4, 'ZERO_DEPOSIT', false, NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(title)
    .bind(message)
    .execute(pool)
    .await;
}

pub async fn notify_document_rejected(
    pool: &PgPool,
    user_id: Uuid,
    doc_label: &str,
    reason: &str,
) {
    let title = "Document Needs Revision ⚠️";
    let message = format!(
        "Your uploaded document '{}' was not accepted: {}. Please re-upload in the redemption chat.",
        doc_label, reason
    );

    let _ = sqlx::query(
        "INSERT INTO notifications (id, user_id, title, message, type, is_read, created_at)
         VALUES ($1, $2, $3, $4, 'ZERO_DEPOSIT', false, NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(title)
    .bind(message)
    .execute(pool)
    .await;
}

pub async fn notify_redemption_approved(
    pool: &PgPool,
    user_id: Uuid,
    landlord_name: &str,
    amount: f64,
) {
    let title = "Deposit Redemption Approved! 🤝";
    let message = format!(
        "Your redemption request of ₹{:.2} for landlord {} has been approved and is queued for payout release.",
        amount, landlord_name
    );

    let _ = sqlx::query(
        "INSERT INTO notifications (id, user_id, title, message, type, is_read, created_at)
         VALUES ($1, $2, $3, $4, 'ZERO_DEPOSIT', false, NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(title)
    .bind(message)
    .execute(pool)
    .await;
}

pub async fn notify_funds_released(
    pool: &PgPool,
    user_id: Uuid,
    amount: f64,
    utr: &str,
) {
    let title = "Deposit Payout Released to Landlord! 🚀";
    let message = format!(
        "₹{:.2} security deposit has been transferred directly to your landlord's bank account. Bank UTR: {}.",
        amount, utr
    );

    let _ = sqlx::query(
        "INSERT INTO notifications (id, user_id, title, message, type, is_read, created_at)
         VALUES ($1, $2, $3, $4, 'ZERO_DEPOSIT', false, NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(title)
    .bind(message)
    .execute(pool)
    .await;
}

