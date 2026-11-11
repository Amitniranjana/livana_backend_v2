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
    services::{
        zd_eligibility_service::{self, ApplicationSignals, EligibilityDecision},
        zd_token_service,
    },
};

type HmacSha256 = Hmac<Sha256>;

const MAX_TIMESTAMP_DRIFT_SECS: i64 = 300; // 5 minutes

// ---------------------------------------------------------------------------
// HMAC verification helper
// ---------------------------------------------------------------------------

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
                Json(json!({ "success": false, "message": "Missing X-Signature header" })),
            )
        })?;

    let timestamp_str = headers
        .get("x-timestamp")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "success": false, "message": "Missing X-Timestamp header" })),
            )
        })?;

    // Validate timestamp age <= 5 minutes
    let now = Utc::now().timestamp();
    let req_timestamp = if let Ok(ts) = timestamp_str.parse::<i64>() {
        ts
    } else if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(timestamp_str) {
        dt.timestamp()
    } else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "success": false, "message": "Invalid X-Timestamp format" })),
        ));
    };

    if (now - req_timestamp).abs() > MAX_TIMESTAMP_DRIFT_SECS {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "success": false, "message": "Request timestamp expired (> 5m drift)" })),
        ));
    }

    // Load secret from DB or env
    let secret = get_hmac_secret(pool).await;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": "HMAC init error" })),
        )
    })?;

    mac.update(body);
    let expected_bytes = mac.finalize().into_bytes();
    let expected_hex = hex::encode(expected_bytes);

    if !subtle_equal(&expected_hex, signature) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "success": false, "message": "Invalid signature" })),
        ));
    }

    Ok(())
}

fn subtle_equal(a: &str, b: &str) -> bool {
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
        "SELECT ai_hmac_secret FROM zd_ai_config LIMIT 1",
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
    pub status: String, // "completed" | "failed"
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
    Path(assessment_id): Path<String>,
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

    // 3. Find assessment record
    let assessment_row = sqlx::query(
        "SELECT id, zero_deposit_id, status FROM zero_deposit_ai_assessments WHERE assessment_id = $1",
    )
    .bind(&assessment_id)
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
        let err_code = payload.error_code.unwrap_or_else(|| "AI_FAILED".into());
        let err_msg = payload.error_message.unwrap_or_else(|| "AI evaluation failed".into());

        sqlx::query(
            r#"UPDATE zero_deposit_ai_assessments
               SET status = 'failed', error_code = $1, error_message = $2, updated_at = NOW()
               WHERE assessment_id = $3"#
        )
        .bind(&err_code)
        .bind(&err_msg)
        .bind(&assessment_id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e.to_string()}))))?;

        // Fall back to ai_manual_review
        sqlx::query(
            "UPDATE zero_deposits SET status = 'ai_manual_review'::zero_deposit_status, updated_at = NOW() WHERE id = $1 AND status = 'ai_review'::zero_deposit_status"
        )
        .bind(zd_id)
        .execute(&state.db)
        .await
        .ok();

        return Ok(Json(json!({ "success": true, "message": "Recorded failed assessment" })));
    }

    // 5. Store extracted, ai_flags, admin_summary
    let ai_flags_json = serde_json::to_value(&payload.ai_flags).unwrap_or(json!([]));
    let rules_ver = payload.rules_version.as_deref().unwrap_or("v2.0");

    sqlx::query(
        r#"UPDATE zero_deposit_ai_assessments
           SET status = 'completed',
               extracted = $1,
               ai_flags = $2,
               admin_summary = $3,
               rules_version = $4,
               updated_at = NOW()
           WHERE assessment_id = $5"#
    )
    .bind(&payload.extracted)
    .bind(&ai_flags_json)
    .bind(&payload.admin_summary)
    .bind(rules_ver)
    .bind(&assessment_id)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e.to_string()}))))?;

    // 6. Gather application signals & run deterministic rules engine
    let signals = gather_signals(&state.db, zd_id, payload.ai_flags).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": format!("Signal gather error: {}", e) })),
        )
    })?;

    let eval_res = zd_eligibility_service::run(&state.db, &signals).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": format!("Rules engine error: {}", e) })),
        )
    })?;

    // 7. Update recommendation on assessment
    let rec_str = match eval_res.decision {
        EligibilityDecision::AutoApprove => "auto_approve",
        EligibilityDecision::ActionRequired => "action_required",
        EligibilityDecision::ManualReview => "manual_review",
        EligibilityDecision::AutoReject => "auto_reject",
    };

    sqlx::query(
        "UPDATE zero_deposit_ai_assessments SET recommendation = $1 WHERE assessment_id = $2",
    )
    .bind(rec_str)
    .bind(&assessment_id)
    .execute(&state.db)
    .await
    .ok();

    // 8. Apply state transitions based on decision
    match eval_res.decision {
        EligibilityDecision::AutoApprove => {
            // Assign tier if computed
            if let Some(tier) = eval_res.tier {
                let _ = zd_eligibility_service::persist_tier(
                    &state.db,
                    zd_id,
                    tier,
                    signals.cibil_score,
                    3.0,
                    35.0,
                    rules_ver,
                )
                .await;
            }

            // Transition from ai_review -> approved -> tokens_issued
            let mut tx = state.db.begin().await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "success": false, "message": e.to_string() })),
                )
            })?;

            let res = sqlx::query(
                "UPDATE zero_deposits SET status = 'approved'::zero_deposit_status, approved_deposit_amount = COALESCE(deposit_amount, requested_deposit_amount), updated_at = NOW() WHERE id = $1 AND status = 'ai_review'::zero_deposit_status",
            )
            .bind(zd_id)
            .execute(&mut *tx)
            .await;

            if let Ok(r) = res {
                if r.rows_affected() > 0 {
                    tx.commit().await.ok();
                    // Issue tokens (3x rent)
                    let rent = signals.monthly_rent;
                    let _ = zd_token_service::issue(
                        &state.db,
                        zd_id,
                        rent,
                        Some("ai_auto"),
                        None,
                        None,
                        Some("Auto-approved by rules engine"),
                    )
                    .await;

                    // Transition to tokens_issued
                    let _ = sqlx::query(
                        "UPDATE zero_deposits SET status = 'tokens_issued'::zero_deposit_status, updated_at = NOW() WHERE id = $1 AND status = 'approved'::zero_deposit_status"
                    )
                    .bind(zd_id)
                    .execute(&state.db)
                    .await;
                }
            }
        }
        EligibilityDecision::ActionRequired => {
            let pending_codes: Vec<String> = eval_res
                .action_required_reasons
                .iter()
                .map(|r| r.code.clone())
                .collect();

            sqlx::query(
                "UPDATE zero_deposits SET status = 'action_required'::zero_deposit_status, pending_actions = $1, updated_at = NOW() WHERE id = $2 AND status = 'ai_review'::zero_deposit_status",
            )
            .bind(&pending_codes)
            .bind(zd_id)
            .execute(&state.db)
            .await
            .ok();
        }
        EligibilityDecision::ManualReview => {
            sqlx::query(
                "UPDATE zero_deposits SET status = 'ai_manual_review'::zero_deposit_status, updated_at = NOW() WHERE id = $1 AND status = 'ai_review'::zero_deposit_status",
            )
            .bind(zd_id)
            .execute(&state.db)
            .await
            .ok();
        }
        EligibilityDecision::AutoReject => {
            sqlx::query(
                "UPDATE zero_deposits SET status = 'rejected'::zero_deposit_status, updated_at = NOW() WHERE id = $1 AND status = 'ai_review'::zero_deposit_status",
            )
            .bind(zd_id)
            .execute(&state.db)
            .await
            .ok();
        }
    }

    Ok(Json(json!({
        "success": true,
        "recommendation": rec_str,
        "tier": eval_res.tier.map(|c| c.to_string())
    })))
}

async fn gather_signals(
    pool: &PgPool,
    zd_id: Uuid,
    ai_flags: Vec<String>,
) -> Result<ApplicationSignals, sqlx::Error> {
    let row = sqlx::query(
        r#"SELECT
            z.id,
            COALESCE(z.monthly_income::FLOAT8, 0.0) as monthly_income,
            COALESCE(z.monthly_rent::FLOAT8, 0.0) as monthly_rent,
            COALESCE(z.deposit_amount::FLOAT8, z.requested_deposit_amount::FLOAT8, 0.0) as deposit_amount,
            COALESCE(z.employment_type, 'salaried') as employment_type,
            z.is_repeat_renter,
            z.bureau_summary
        FROM zero_deposits z
        WHERE z.id = $1"#,
    )
    .bind(zd_id)
    .fetch_one(pool)
    .await?;

    let monthly_income: f64 = row.get("monthly_income");
    let monthly_rent: f64 = row.get("monthly_rent");
    let deposit_amount: f64 = row.get("deposit_amount");
    let employment_type: String = row.get("employment_type");
    let is_repeat_renter: bool = row.get("is_repeat_renter");
    let bureau_json: Value = row.try_get("bureau_summary").unwrap_or(json!({}));

    // Parse bureau signals
    let cibil_score = bureau_json.get("score").and_then(|v| v.as_i64()).map(|v| v as i32);
    let dpd_12m = bureau_json.get("dpd_12m").and_then(|v| v.as_i64()).map(|v| v as i32);
    let dpd_24m = bureau_json.get("dpd_24m").and_then(|v| v.as_i64()).map(|v| v as i32);
    let enquiries_3m = bureau_json.get("enquiries_3m").and_then(|v| v.as_i64()).map(|v| v as i32);
    let emi_total = bureau_json.get("emi_total_monthly").and_then(|v| v.as_f64());
    let card_dues = bureau_json.get("card_dues").and_then(|v| v.as_f64());
    let write_offs = bureau_json.get("write_offs").and_then(|v| v.as_i64()).map(|v| v as i32);
    let settlements = bureau_json.get("settlements").and_then(|v| v.as_i64()).map(|v| v as i32);
    let suit_filed = bureau_json.get("suit_filed").and_then(|v| v.as_bool()).unwrap_or(false);

    // Bank statement analysis row
    let bank_row = sqlx::query(
        "SELECT bounced_cheque_count, avg_monthly_credit::FLOAT8, high_debt_load_flag FROM bank_statement_analyses WHERE zero_deposit_id = $1 ORDER BY created_at DESC LIMIT 1"
    )
    .bind(zd_id)
    .fetch_optional(pool)
    .await?;

    let (bounced_cheque_count, avg_monthly_credit, high_debt_load_flag) = match bank_row {
        Some(r) => (
            r.try_get::<i32, _>("bounced_cheque_count").ok(),
            r.try_get::<f64, _>("avg_monthly_credit").ok(),
            r.try_get::<bool, _>("high_debt_load_flag").unwrap_or(false),
        ),
        None => (None, None, false),
    };

    // Co-applicant
    let co_row = sqlx::query(
        "SELECT monthly_income::FLOAT8, bureau_summary FROM zd_co_applicants WHERE zero_deposit_id = $1 AND consent_given = true LIMIT 1"
    )
    .bind(zd_id)
    .fetch_optional(pool)
    .await?;

    let (co_applicant_income, co_applicant_cibil) = match co_row {
        Some(r) => {
            let inc: Option<f64> = r.try_get("monthly_income").ok();
            let b_val: Value = r.try_get("bureau_summary").unwrap_or(json!({}));
            let cibil = b_val.get("score").and_then(|v| v.as_i64()).map(|v| v as i32);
            (inc, cibil)
        }
        None => (None, None),
    };

    // Landlord reference
    let landlord_status = sqlx::query_scalar::<_, String>(
        "SELECT reference_status FROM zd_landlord_references WHERE zero_deposit_id = $1 ORDER BY created_at DESC LIMIT 1"
    )
    .bind(zd_id)
    .fetch_optional(pool)
    .await?;

    Ok(ApplicationSignals {
        application_id: zd_id,
        monthly_income,
        monthly_rent,
        deposit_amount,
        employment_type,
        cibil_score,
        dpd_12m,
        dpd_24m,
        enquiries_3m,
        emi_total,
        card_dues,
        write_offs,
        settlements,
        suit_filed,
        bounced_cheque_count,
        avg_monthly_credit,
        high_debt_load_flag,
        kyc_fraud_flag: false,
        is_repeat_renter,
        landlord_reference_status: landlord_status,
        co_applicant_income,
        co_applicant_cibil,
        ai_flags,
    })
}

// ---------------------------------------------------------------------------
// Issue 115: Document Check Result Callback
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct DocumentCheckResultCallback {
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

    // 3. Find redemption with doc_check_id = check_id
    let redemption = sqlx::query(
        "SELECT id, zero_deposit_id, status FROM zd_redemptions WHERE doc_check_id = $1",
    )
    .bind(&check_id)
    .fetch_optional(&state.db)
    .await
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
                Json(json!({ "success": false, "message": "Redemption check not found" })),
            ));
        }
    };

    let redemption_id: Uuid = redemption.get("id");
    let current_status: String = redemption.get("status");

    // Idempotent: don't overwrite if already processed beyond doc_check
    if current_status == "approved" || current_status == "completed" || current_status == "rejected" {
        return Ok(Json(json!({ "success": true, "message": "Redemption already finalized" })));
    }

    // 4. Validate flags against the supported spec flags:
    // AGREEMENT_TENANT_MISMATCH, AGREEMENT_ADDRESS_MISMATCH, AGREEMENT_RENT_MISMATCH,
    // AGREEMENT_TERM_BELOW_11M, NO_REFUND_ASSIGNMENT, DEPOSIT_NOT_3X_RENT,
    // OCCUPANTS_EXCEED_AGREEMENT, AGREEMENT_UNSIGNED, PAYEE_NAME_MISMATCH,
    // LANDLORD_KYC_MISMATCH, OWNER_MISMATCH.
    let flags_json = serde_json::to_value(&payload.flags).unwrap_or(json!([]));

    // 5. Update redemption with doc_check_flags and advance status to admin_review
    sqlx::query(
        r#"UPDATE zd_redemptions
           SET doc_check_flags = $1,
               status = 'admin_review'::zd_redemption_status,
               admin_remarks = COALESCE($2, admin_remarks),
               updated_at = NOW()
           WHERE id = $3"#
    )
    .bind(&flags_json)
    .bind(&payload.admin_notes)
    .bind(redemption_id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": e.to_string() })),
        )
    })?;

    // 6. Update document verified flags if provided
    if let Some(ag_verified) = payload.agreement_verified {
        sqlx::query(
            "UPDATE zd_redemption_documents SET verified = $1 WHERE redemption_id = $2 AND doc_type = 'agreement'",
        )
        .bind(ag_verified)
        .bind(redemption_id)
        .execute(&state.db)
        .await
        .ok();
    }

    if let Some(bp_verified) = payload.bank_proof_verified {
        sqlx::query(
            "UPDATE zd_redemption_documents SET verified = $1 WHERE redemption_id = $2 AND doc_type = 'bank_proof'",
        )
        .bind(bp_verified)
        .bind(redemption_id)
        .execute(&state.db)
        .await
        .ok();
    }

    Ok(Json(json!({
        "success": true,
        "message": "Document check result recorded; forwarded to admin_review",
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
