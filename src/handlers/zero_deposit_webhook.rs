use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    body::Bytes,
    Json,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde_json::json;
use sqlx::Row;

use crate::{
    app_state::AppState,
    models::zero_deposit::{
        RepaymentWebhookRequest, FeeWebhookRequest,
        FeeTransactionStatus, RepaymentStatus, LedgerTransactionType,
    },
};

fn verify_webhook_signature(headers: &HeaderMap, body: &[u8], secret: &str) -> bool {
    let signature = match headers.get("x-webhook-signature") {
        Some(val) => val.to_str().unwrap_or(""),
        None => return false,
    };

    let mut mac = match Hmac::<Sha256>::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(body);
    let result = mac.finalize().into_bytes();
    let expected = hex::encode(result);

    signature == expected
}

/// POST /api/v1/zero-deposit/webhook/repayment-status
pub async fn repayment_webhook(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl axum::response::IntoResponse {
    let webhook_secret = std::env::var("ZD_WEBHOOK_SECRET")
        .or_else(|_| std::env::var("NBFC_WEBHOOK_SECRET"))
        .unwrap_or_else(|_| "secret".to_string());
    if !verify_webhook_signature(&headers, &body, &webhook_secret) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"success": false, "message": "Invalid signature"})));
    }

    let payload: RepaymentWebhookRequest = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(e) => {
            log::error!("Invalid repayment payload: {}", e);
            return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "Invalid payload"})));
        }
    };

    let (status_enum, is_defaulted) = match payload.status.to_lowercase().as_str() {
        "paid" | "success" => (RepaymentStatus::Paid, false),
        "missed" | "failed" | "overdue" => (RepaymentStatus::Overdue, true),
        _ => (RepaymentStatus::Pending, false),
    };

    let mut tx = match app_state.db.begin().await {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": "DB error"}))),
    };

    let partner_repayment_id_str = payload.repayment_id.to_string();

    match sqlx::query(
        "UPDATE repayment_schedules SET status = $1::repayment_status, partner_repayment_id = $2, amount_paid = amount_paid + $3, paid_at = $4, updated_at = NOW() WHERE zero_deposit_id = $5 AND id = $6"
    )
    .bind(status_enum.clone() as RepaymentStatus)
    .bind(&partner_repayment_id_str)
    .bind(payload.amount_paid)
    .bind(payload.paid_at)
    .bind(payload.zero_deposit_id)
    .bind(payload.repayment_id)
    .execute(&mut *tx)
    .await {
        Ok(_) => {},
        Err(e) => log::error!("repayment_webhook update error: {}", e),
    };

    if status_enum == RepaymentStatus::Paid {
        match sqlx::query(
            "INSERT INTO ledger_entries (zero_deposit_id, transaction_type, amount, reference_id, description) VALUES ($1, $2::ledger_transaction_type, $3, $4, $5)"
        )
        .bind(payload.zero_deposit_id)
        .bind(LedgerTransactionType::Repayment)
        .bind(payload.amount_paid)
        .bind(&partner_repayment_id_str)
        .bind("Monthly Repayment")
        .execute(&mut *tx)
        .await {
            Ok(_) => {},
            Err(e) => {
                log::error!("repayment_webhook ledger error: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": "DB error"})));
            }
        };
    }

    if is_defaulted {
        match sqlx::query(
            "UPDATE zero_deposits SET is_defaulted = true, updated_at = NOW() WHERE id = $1"
        )
        .bind(payload.zero_deposit_id)
        .execute(&mut *tx)
        .await {
            Ok(_) => {},
            Err(e) => log::error!("repayment_webhook default update error: {}", e),
        };
    }

    if tx.commit().await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": "DB error"})));
    }

    (StatusCode::OK, Json(json!({"success": true})))
}

/// POST /api/v1/zero-deposit/webhook/fee-status
pub async fn fee_webhook(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl axum::response::IntoResponse {
    let webhook_secret = std::env::var("ZD_WEBHOOK_SECRET")
        .or_else(|_| std::env::var("NBFC_WEBHOOK_SECRET"))
        .unwrap_or_else(|_| "secret".to_string());
    if !verify_webhook_signature(&headers, &body, &webhook_secret) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"success": false, "message": "Invalid signature"})));
    }

    let payload: FeeWebhookRequest = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(e) => {
            log::error!("Invalid fee payload: {}", e);
            return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "Invalid payload"})));
        }
    };

    let status_enum = match payload.status.to_lowercase().as_str() {
        "success" | "paid" => FeeTransactionStatus::Success,
        "failed" => FeeTransactionStatus::Failed,
        _ => FeeTransactionStatus::Pending,
    };

    let mut tx = match app_state.db.begin().await {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": "DB error"}))),
    };

    match sqlx::query(
        "UPDATE fee_transactions SET status = $1::fee_transaction_status, upi_txn_ref = $2, updated_at = NOW() WHERE zero_deposit_id = $3"
    )
    .bind(status_enum.clone() as FeeTransactionStatus)
    .bind(&payload.upi_txn_ref)
    .bind(payload.zero_deposit_id)
    .execute(&mut *tx)
    .await {
        Ok(_) => {},
        Err(e) => {
            log::error!("fee_webhook update error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": "DB error"})));
        }
    };

    if status_enum == FeeTransactionStatus::Success {
        let fee_amount: f64 = match sqlx::query(
            "SELECT amount::FLOAT8 as amount FROM fee_transactions WHERE zero_deposit_id = $1"
        )
        .bind(payload.zero_deposit_id)
        .fetch_optional(&mut *tx)
        .await
        {
            Ok(Some(row)) => row.get("amount"),
            _ => 500.0,
        };

        match sqlx::query(
            "INSERT INTO ledger_entries (zero_deposit_id, transaction_type, amount, reference_id, description) VALUES ($1, $2::ledger_transaction_type, $3, $4, $5)"
        )
        .bind(payload.zero_deposit_id)
        .bind(LedgerTransactionType::Fee)
        .bind(fee_amount)
        .bind(&payload.upi_txn_ref)
        .bind("Processing Fee")
        .execute(&mut *tx)
        .await {
            Ok(_) => {},
            Err(e) => {
                log::error!("fee_webhook ledger error: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": "DB error"})));
            }
        };

        // Note: Issue 108 — Fee payment does not overwrite application status
    }

    if tx.commit().await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": "DB error"})));
    }

    (StatusCode::OK, Json(json!({"success": true})))
}
