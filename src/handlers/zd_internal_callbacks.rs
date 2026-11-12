//! Issues 113 & 115 — Internal Callbacks for AI Services
//!
//! Internal endpoints:
//! - POST /internal/zero-deposit/assessments/{assessment_id}/result
//! - POST /internal/zero-deposit/document-checks/{check_id}/result
//!
//! HMAC Verification:
//! - HMAC-SHA256 over raw request body using secret configured in `zd_ai_config` or `ZD_AI_HMAC_SECRET` env var.
//! - Headers: `X-Signature` (hex-encoded HMAC) and `X-Timestamp` (unix epoch seconds or ISO8601).
//! - Rejects signatures older than 5 minutes (300 seconds) or with invalid HMAC with HTTP 401.

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::Sha256;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    services::zd_eligibility_service::{self, ExtractedBankData},
};

type HmacSha256 = Hmac<Sha256>;

pub const MAX_TIMESTAMP_DRIFT_SECS: i64 = 300; // 5 minutes

// ---------------------------------------------------------------------------
// HMAC verification helper
// ---------------------------------------------------------------------------

pub fn compute_hmac(secret: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC init should not fail");
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

pub fn verify_hmac_raw(
    secret: &str,
    signature: &str,
    timestamp_str: &str,
    body: &[u8],
    current_time_secs: i64,
) -> Result<(), &'static str> {
    let req_timestamp = if let Ok(ts) = timestamp_str.parse::<i64>() {
        ts
    } else if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(timestamp_str) {
        dt.timestamp()
    } else {
        return Err("Invalid X-Timestamp format");
    };

    if (current_time_secs - req_timestamp).abs() > MAX_TIMESTAMP_DRIFT_SECS {
        return Err("Request timestamp expired (> 5m drift)");
    }

    let expected_hex = compute_hmac(secret, body);
    if !subtle_equal(&expected_hex, signature) {
        return Err("Invalid signature");
    }

    Ok(())
}

async fn verify_hmac(
    pool: &PgPool,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<(), (StatusCode, Json<Value>)> {
    let signature = headers
        .get("x-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "success": false, "error": "BAD_HMAC", "message": "Missing X-Signature header" })),
            )
        })?;

    let timestamp_str = headers
        .get("x-timestamp")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "success": false, "error": "BAD_HMAC", "message": "Missing X-Timestamp header" })),
            )
        })?;

    let secret = get_hmac_secret(pool).await;
    let now = Utc::now().timestamp();

    if let Err(msg) = verify_hmac_raw(&secret, signature, timestamp_str, body, now) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "success": false, "error": "BAD_HMAC", "message": msg })),
        ));
    }

    Ok(())
}

pub fn subtle_equal(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

async fn get_hmac_secret(pool: &PgPool) -> String {
    if let Ok(sec) = std::env::var("ZD_AI_HMAC_SECRET") {
        if !sec.is_empty() {
            return sec;
        }
    }

    let row = sqlx::query_scalar::<_, String>(
        "SELECT config->>'ai_hmac_secret' FROM zd_ai_config LIMIT 1",
    )
    .fetch_optional(pool)
    .await;

    match row {
        Ok(Some(s)) if !s.is_empty() => s,
        _ => "change-me-in-production".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Issue 113: Assessment Result Callback
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct AssessmentResultCallback {
    pub assessment_id: Option<String>,
    pub status: String, // "completed" | "failed"
    pub model_version: Option<String>,
    #[serde(default)]
    pub extracted: Value,
    #[serde(default)]
    pub ai_flags: Vec<String>,
    pub admin_summary: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub rules_version: Option<String>,
}

pub async fn assessment_result_callback(
    State(state): State<AppState>,
    Path(path_assessment_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // 1. Verify HMAC
    verify_hmac(&state.db, &headers, &body).await?;

    // 2. Parse payload
    let payload: AssessmentResultCallback = serde_json::from_slice(&body).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "message": format!("Invalid payload: {}", e) })),
        )
    })?;

    let assessment_id_str = payload.assessment_id.as_deref().unwrap_or(&path_assessment_id);
    let assessment_uuid = Uuid::parse_str(assessment_id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "message": "Invalid assessment_id format (must be UUID)" })),
        )
    })?;

    // 3. Find assessment record
    let assessment_row = sqlx::query(
        "SELECT id, zero_deposit_id, status::TEXT as status FROM zero_deposit_ai_assessments WHERE id = $1",
    )
    .bind(assessment_uuid)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": e.to_string() })),
        )
    })?;

    let row = match assessment_row {
        Some(r) => r,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "message": "Assessment not found" })),
            ));
        }
    };

    let zd_id: Uuid = row.get("zero_deposit_id");
    let current_status: String = row.get("status");

    // Idempotency check: if already completed, return 200 OK immediately
    if current_status == "completed" {
        return Ok(Json(json!({ "success": true, "message": "Already processed" })));
    }

    // 4. Handle "failed" status
    if payload.status == "failed" {
        let err_code = payload.error_code.unwrap_or_else(|| "AI_UNAVAILABLE".into());

        sqlx::query(
            r#"UPDATE zero_deposit_ai_assessments
               SET status = 'failed', error_code = $1, completed_at = NOW()
               WHERE id = $2"#
        )
        .bind(&err_code)
        .bind(assessment_uuid)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e.to_string()}))))?;

        // Fall back to ai_manual_review with AI_UNAVAILABLE
        sqlx::query(
            "UPDATE zero_deposits SET status = 'ai_manual_review'::zero_deposit_status, updated_at = NOW() WHERE id = $1 AND status = 'ai_review'::zero_deposit_status"
        )
        .bind(zd_id)
        .execute(&state.db)
        .await
        .ok();

        return Ok(Json(json!({ "success": true, "message": "Recorded failed assessment" })));
    }

    // 5. Parse extracted bank data
    let extracted_data = ExtractedBankData {
        net_take_home_monthly: payload.extracted.get("net_take_home_monthly").and_then(|v| v.as_f64()),
        variable_pay_monthly: payload.extracted.get("variable_pay_monthly").and_then(|v| v.as_f64()),
        variable_pay_months_seen: payload.extracted.get("variable_pay_months_seen").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        avg_monthly_credits_6m: payload.extracted.get("avg_monthly_credits_6m").and_then(|v| v.as_f64()),
        salary_regular: payload.extracted.get("salary_regular").and_then(|v| v.as_bool()),
        bounces_3m: payload.extracted.get("bounces_3m").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        avg_monthly_balance: payload.extracted.get("avg_monthly_balance").and_then(|v| v.as_f64()).unwrap_or(0.0),
        overdraft_days_6m: payload.extracted.get("overdraft_days_6m").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        unexplained_cash_max_month: payload.extracted.get("unexplained_cash_max_month").and_then(|v| v.as_f64()).unwrap_or(0.0),
        loan_app_count: payload.extracted.get("loan_app_count").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        emi_debits_monthly: payload.extracted.get("emi_debits_monthly").and_then(|v| v.as_f64()),
    };

    // 6. Run deterministic rules engine
    let eval_res = zd_eligibility_service::run_rules_engine_for_application(
        &state.db,
        zd_id,
        &extracted_data,
        &payload.ai_flags,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": format!("Rules engine error: {}", e) })),
        )
    })?;

    // 7. Persist assessment row
    let model_version = payload.model_version.as_deref().unwrap_or("zd-extract-v1.0");
    let admin_summary = payload.admin_summary.as_deref().unwrap_or(&eval_res.admin_summary);

    let bureau_summary = sqlx::query_scalar::<_, Value>(
        "SELECT bureau_summary FROM zero_deposits WHERE id = $1"
    )
    .bind(zd_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or(json!({}));

    zd_eligibility_service::persist_assessment(
        &state.db,
        zd_id,
        Some(assessment_uuid),
        model_version,
        &eval_res,
        &payload.extracted,
        &bureau_summary,
        admin_summary,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": format!("Persist assessment error: {}", e) })),
        )
    })?;

    // 8. Apply state transitions based on decision
    let app_meta = sqlx::query(
        "SELECT user_id, monthly_rent::FLOAT8 as rent FROM zero_deposits WHERE id = $1"
    )
    .bind(zd_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": e.to_string() })),
        )
    })?;

    let user_id: Uuid = app_meta.get("user_id");
    let monthly_rent: f64 = app_meta.get("rent");

    zd_eligibility_service::apply_assessment_decision(
        &state.db,
        zd_id,
        user_id,
        assessment_uuid,
        &eval_res,
        monthly_rent,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": format!("Apply decision error: {}", e) })),
        )
    })?;

    // Send push notification if active websocket exists
    if let Some(sender) = state.active_sockets.get(&user_id) {
        let msg = json!({
            "type": "zd_assessment_update",
            "zero_deposit_id": zd_id,
            "decision": format!("{:?}", eval_res.decision),
            "tier": eval_res.tier.map(|t| t.as_str()),
            "message": eval_res.tenant_message
        });
        let _ = sender.send(msg.to_string()).await;
    }

    Ok(Json(json!({
        "success": true,
        "message": "Assessment processed successfully",
        "decision": format!("{:?}", eval_res.decision),
        "tier": eval_res.tier.map(|t| t.as_str())
    })))
}

// ---------------------------------------------------------------------------
// Document Check Result Callback (Issue 115)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct DocumentCheckResultCallback {
    pub check_id: Option<String>,
    pub redemption_id: Option<Uuid>,
    #[serde(default)]
    pub agreement: Option<Value>,
    #[serde(default)]
    pub bank_proof: Option<Value>,
    #[serde(default)]
    pub flags: Vec<String>,
    pub agreement_verified: Option<bool>,
    pub bank_proof_verified: Option<bool>,
    pub extracted_details: Option<Value>,
    pub admin_notes: Option<String>,
}

pub async fn document_check_result_callback(
    State(state): State<AppState>,
    Path(check_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // 1. Verify HMAC
    verify_hmac(&state.db, &headers, &body).await?;

    // 2. Parse payload
    let payload: DocumentCheckResultCallback = serde_json::from_slice(&body).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "message": format!("Invalid payload: {}", e) })),
        )
    })?;

    // 3. Find redemption
    let redemption = if let Some(rid) = payload.redemption_id {
        sqlx::query(
            "SELECT id, user_id, chat_id, zero_deposit_id, status::TEXT as status FROM zd_redemptions WHERE id = $1",
        )
        .bind(rid)
        .fetch_optional(&state.db)
        .await
    } else {
        sqlx::query(
            "SELECT id, user_id, chat_id, zero_deposit_id, status::TEXT as status FROM zd_redemptions WHERE doc_check_id = $1 OR id::TEXT = $1",
        )
        .bind(&check_id)
        .fetch_optional(&state.db)
        .await
    }
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": e.to_string() })),
        )
    })?;

    let redemption = match redemption {
        Some(r) => r,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "message": "Redemption not found for document check" })),
            ));
        }
    };

    let redemption_id: Uuid = redemption.get("id");
    let user_id: Uuid = redemption.get("user_id");
    let chat_id: Uuid = redemption.get("chat_id");
    let current_status: String = redemption.get("status");

    if current_status == "approved" || current_status == "released" || current_status == "rejected" || current_status == "cancelled" {
        return Ok(Json(json!({ "success": true, "message": "Redemption already finalized" })));
    }

    // 4. Update redemption with doc check results
    let check_result = json!({
        "check_id": check_id,
        "agreement": payload.agreement,
        "bank_proof": payload.bank_proof,
        "flags": payload.flags,
        "extracted_details": payload.extracted_details,
        "agreement_verified": payload.agreement_verified,
        "bank_proof_verified": payload.bank_proof_verified,
        "admin_notes": payload.admin_notes
    });

    sqlx::query(
        r#"UPDATE zd_redemptions
           SET ai_doc_check_result = $1,
               doc_check_flags = $2,
               doc_check_id = $3,
               updated_at = NOW()
           WHERE id = $4"#
    )
    .bind(&check_result)
    .bind(json!(payload.flags))
    .bind(&check_id)
    .bind(redemption_id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": e.to_string() })),
        )
    })?;

    // 5. Post system message to redemption chat
    let rent_match = payload.agreement.as_ref()
        .and_then(|a| a.get("monthly_rent"))
        .and_then(|r| r.as_f64());
    let rent_summary = if let Some(r) = rent_match {
        format!(", rent matches ₹{:.0}", r)
    } else {
        "".to_string()
    };
    let flags_summary = if payload.flags.is_empty() {
        "No flags detected.".to_string()
    } else {
        format!("{} flag(s): {}", payload.flags.len(), payload.flags.join(", "))
    };
    let summary_content = format!(
        "AI doc check: Agreement analyzed{}{}. Flags: {}",
        rent_summary,
        if payload.flags.is_empty() { "" } else { " with warnings" },
        flags_summary
    );

    let msg_id = Uuid::new_v4();
    let _ = sqlx::query(
        r#"INSERT INTO messages (id, chat_id, sender_type, message_type, content, metadata, created_at)
           VALUES ($1, $2, 'system', 'doc_check', $3, $4, NOW())"#
    )
    .bind(msg_id)
    .bind(chat_id)
    .bind(&summary_content)
    .bind(json!({ "doc_check_flags": payload.flags, "check_id": check_id }))
    .execute(&state.db)
    .await;

    let msg_val = json!({
        "id": msg_id,
        "chat_id": chat_id,
        "sender_type": "system",
        "message_type": "doc_check",
        "content": summary_content,
        "metadata": { "doc_check_flags": payload.flags },
        "created_at": Utc::now()
    });
    crate::services::zd_redemption_service::broadcast_redemption_message(&state, chat_id, Some(user_id), &msg_val).await;

    Ok(Json(json!({
        "success": true,
        "message": "Document check result recorded",
        "redemption_id": redemption_id,
        "flags_count": payload.flags.len()
    })))
}

// ---------------------------------------------------------------------------
// Route definitions
// ---------------------------------------------------------------------------

pub fn internal_routes() -> axum::Router<AppState> {
    use axum::routing::post;

    axum::Router::new()
        .route(
            "/internal/zero-deposit/assessments/{assessment_id}/result",
            post(assessment_result_callback),
        )
        .route(
            "/internal/zero-deposit/document-checks/{check_id}/result",
            post(document_check_result_callback),
        )
}

// ---------------------------------------------------------------------------
// HMAC Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_hmac_passes() {
        let secret = "test-secret-key-12345";
        let body = b"{\"assessment_id\":\"test-123\",\"status\":\"completed\"}";
        let now = Utc::now().timestamp();
        let timestamp_str = now.to_string();

        let sig = compute_hmac(secret, body);

        let res = verify_hmac_raw(secret, &sig, &timestamp_str, body, now);
        assert!(res.is_ok());
    }

    #[test]
    fn test_bad_signature_fails() {
        let secret = "test-secret-key-12345";
        let body = b"{\"assessment_id\":\"test-123\",\"status\":\"completed\"}";
        let now = Utc::now().timestamp();
        let timestamp_str = now.to_string();

        let bad_sig = "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";

        let res = verify_hmac_raw(secret, bad_sig, &timestamp_str, body, now);
        assert_eq!(res, Err("Invalid signature"));
    }

    #[test]
    fn test_stale_timestamp_fails() {
        let secret = "test-secret-key-12345";
        let body = b"{\"assessment_id\":\"test-123\",\"status\":\"completed\"}";
        let now = Utc::now().timestamp();
        let stale_timestamp = (now - 301).to_string(); // 301s ago (> 5 mins)

        let sig = compute_hmac(secret, body);

        let res = verify_hmac_raw(secret, &sig, &stale_timestamp, body, now);
        assert_eq!(res, Err("Request timestamp expired (> 5m drift)"));
    }
}
