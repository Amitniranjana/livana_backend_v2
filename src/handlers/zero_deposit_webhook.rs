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

    // Idempotency: check if repayment is already paid
    let current_repayment_status = sqlx::query_scalar::<_, String>(
        "SELECT status::TEXT FROM repayment_schedules WHERE zero_deposit_id = $1 AND id = $2"
    )
    .bind(payload.zero_deposit_id)
    .bind(payload.repayment_id)
    .fetch_optional(&mut *tx)
    .await
    .unwrap_or(None);

    if current_repayment_status.as_deref() == Some("paid") && status_enum == RepaymentStatus::Paid {
        return (StatusCode::OK, Json(json!({"success": true, "message": "Repayment already recorded"})));
    }

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

    // Tier rule: late payment downgrades tier and resets on_time_months; on-time increments on_time_months
    let user_id = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT user_id FROM zero_deposits WHERE id = $1"
    )
    .bind(payload.zero_deposit_id)
    .fetch_optional(&mut *tx)
    .await
    .unwrap_or(None);

    let is_late = is_defaulted || match (payload.paid_at, payload.due_date) {
        (Some(paid), Some(due)) => paid.date_naive() > due,
        _ => false,
    };

    if let Some(uid) = user_id {
        if is_late {
            let _ = sqlx::query(
                r#"UPDATE zd_tenant_tiers
                   SET tier = CASE WHEN tier::TEXT = 'A' THEN 'B'::zd_tenant_tier ELSE 'C'::zd_tenant_tier END,
                       on_time_months = 0,
                       source = 'auto_downgrade',
                       reason = 'Late repayment recorded',
                       updated_at = NOW()
                   WHERE user_id = $1 AND source != 'admin'"#
            )
            .bind(uid)
            .execute(&mut *tx)
            .await;
        } else if status_enum == RepaymentStatus::Paid {
            let tier_row = sqlx::query(
                "SELECT tier::TEXT as tier, on_time_months FROM zd_tenant_tiers WHERE user_id = $1"
            )
            .bind(uid)
            .fetch_optional(&mut *tx)
            .await
            .unwrap_or(None);

            if let Some(row) = tier_row {
                let tier: String = row.get("tier");
                let on_time: i32 = row.get("on_time_months");
                let new_on_time = on_time + 1;

                if new_on_time >= 6 {
                    let new_tier = match tier.as_str() {
                        "C" => "B",
                        "B" => "A",
                        _ => "A",
                    };
                    let _ = sqlx::query(
                        r#"UPDATE zd_tenant_tiers
                           SET tier = $1::zd_tenant_tier,
                               source = 'auto_upgrade',
                               on_time_months = 0,
                               reason = '6 months on-time rent qualification',
                               updated_at = NOW()
                           WHERE user_id = $2 AND source != 'admin'"#
                    )
                    .bind(new_tier)
                    .bind(uid)
                    .execute(&mut *tx)
                    .await;
                } else {
                    let _ = sqlx::query(
                        "UPDATE zd_tenant_tiers SET on_time_months = $1, updated_at = NOW() WHERE user_id = $2 AND source != 'admin'"
                    )
                    .bind(new_on_time)
                    .bind(uid)
                    .execute(&mut *tx)
                    .await;
                }
            }
        }
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

    // Idempotency: check if already marked success
    let current_fee_status = sqlx::query_scalar::<_, String>(
        "SELECT status::TEXT FROM fee_transactions WHERE zero_deposit_id = $1 ORDER BY created_at DESC LIMIT 1"
    )
    .bind(payload.zero_deposit_id)
    .fetch_optional(&mut *tx)
    .await
    .unwrap_or(None);

    if current_fee_status.as_deref() == Some("success") && status_enum == FeeTransactionStatus::Success {
        return (StatusCode::OK, Json(json!({"success": true, "message": "Fee already recorded"})));
    }

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
            _ => 12000.0,
        };

        match sqlx::query(
            "INSERT INTO ledger_entries (zero_deposit_id, transaction_type, amount, reference_id, description) VALUES ($1, $2::ledger_transaction_type, $3, $4, $5)"
        )
        .bind(payload.zero_deposit_id)
        .bind(LedgerTransactionType::Fee)
        .bind(fee_amount)
        .bind(&payload.upi_txn_ref)
        .bind("Processing Fee + First Month Rent")
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

// ---------------------------------------------------------------------------
// E-Sign Provider & Webhook
// ---------------------------------------------------------------------------

pub trait EsignProvider: Send + Sync {
    fn verify_signature(&self, headers: &HeaderMap, body: &[u8], secret: &str) -> bool;
    fn parse_event(&self, body: &[u8]) -> Result<EsignWebhookRequest, String>;
}

pub struct StubEsignProvider;

impl EsignProvider for StubEsignProvider {
    fn verify_signature(&self, headers: &HeaderMap, body: &[u8], secret: &str) -> bool {
        verify_webhook_signature(headers, body, secret)
    }

    fn parse_event(&self, body: &[u8]) -> Result<EsignWebhookRequest, String> {
        serde_json::from_slice(body).map_err(|e| e.to_string())
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct EsignWebhookRequest {
    pub zero_deposit_id: uuid::Uuid,
    pub signer_role: String, // "tenant" | "co_applicant"
    pub status: String,      // "signed" | "declined" | "expired"
    pub document_id: Option<String>,
    pub signed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// POST /api/v1/zero-deposit/webhook/esign-status
pub async fn esign_webhook(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl axum::response::IntoResponse {
    let webhook_secret = std::env::var("ZD_WEBHOOK_SECRET")
        .or_else(|_| std::env::var("NBFC_WEBHOOK_SECRET"))
        .unwrap_or_else(|_| "secret".to_string());

    let provider = StubEsignProvider;
    if !provider.verify_signature(&headers, &body, &webhook_secret) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"success": false, "message": "Invalid signature"})));
    }

    let payload = match provider.parse_event(&body) {
        Ok(p) => p,
        Err(e) => {
            log::error!("Invalid esign payload: {}", e);
            return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "Invalid payload"})));
        }
    };

    let mut tx = match app_state.db.begin().await {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": "DB error"}))),
    };

    if payload.status.to_lowercase() == "signed" {
        if payload.signer_role.to_lowercase() == "co_applicant" {
            let _ = sqlx::query(
                "UPDATE zd_co_applicants SET esigned_at = NOW(), updated_at = NOW() WHERE zero_deposit_id = $1 AND status = 'accepted'"
            )
            .bind(payload.zero_deposit_id)
            .execute(&mut *tx)
            .await;
        }

        // Post a status message in the redemption chat if chat exists
        let chat_id = sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT id FROM chats WHERE zd_redemption_id IN (SELECT id FROM zd_redemptions WHERE zero_deposit_id = $1) LIMIT 1"
        )
        .bind(payload.zero_deposit_id)
        .fetch_optional(&mut *tx)
        .await
        .unwrap_or(None);

        if let Some(cid) = chat_id {
            let content = format!("Agreement e-signed by {}.", payload.signer_role);
            let metadata = json!({
                "signer_role": payload.signer_role,
                "status": "signed",
                "document_id": payload.document_id,
            });

            let _ = sqlx::query(
                r#"INSERT INTO messages (id, chat_id, sender_type, message_type, content, metadata, created_at)
                   VALUES (gen_random_uuid(), $1, 'system', 'status', $2, $3, NOW())"#
            )
            .bind(cid)
            .bind(content)
            .bind(metadata)
            .execute(&mut *tx)
            .await;
        }
    }

    if tx.commit().await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": "DB error"})));
    }

    (StatusCode::OK, Json(json!({"success": true, "message": "Esign status updated"})))
}
