/// Issue 93 — GET /api/admin/loans/{loan_id}/risk-score
/// Issue 94 — POST /api/v1/loans/{loan_id}/bank-statement/analyze
/// Issue 95 — Extend approve/reject endpoints with decision_reasons

use axum::{
    extract::{Path, State, Extension},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::loan_risk::DecisionReason,
    handlers::admin_auth::AdminClaims,
    services::loan_risk_service,
    utils::admin_logger::log_admin_action,
};

// ─── Issue 93: GET /api/admin/loans/{loan_id}/risk-score ──────────────────

/// Admin endpoint: compute (or re-compute) the probability-of-default risk
/// score for a loan/zero-deposit application.
///
/// Design note: this endpoint intentionally re-runs the scoring engine on
/// every call so that scores stay fresh if the bank statement analysis was
/// updated since the last score. A caching layer (Redis TTL) can be added
/// in a future iteration without API change.
pub async fn get_loan_risk_score(
    State(state): State<AppState>,
    Path(loan_id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let score = loan_risk_service::score_loan_application(&state.db, loan_id)
        .await
        .map_err(|e| {
            let code = if e.contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (code, Json(json!({ "success": false, "message": e })))
        })?;

    Ok(Json(json!({
        "success": true,
        "data": score
    })))
}

// ─── Issue 94: POST /api/v1/loans/{loan_id}/bank-statement/analyze ─────────

/// System-triggered (or manually triggered) bank-statement analysis endpoint.
///
/// In v1 this runs a rule-based analysis on the income / rent figures already
/// stored on the zero_deposit row (the actual PDF parsing is a TODO for v2).
/// The result is persisted in `bank_statement_analyses` and a fresh risk score
/// is computed and stored in `loan_risk_scores`.
pub async fn analyze_bank_statement(
    State(state): State<AppState>,
    Path(loan_id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Run the bank statement analysis
    let _analysis_id = loan_risk_service::analyze_bank_statement(&state.db, loan_id, None)
        .await
        .map_err(|e| {
            let code = if e.contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (code, Json(json!({ "success": false, "message": e })))
        })?;

    // After analysis, recompute the risk score so it reflects the new data
    let score = loan_risk_service::score_loan_application(&state.db, loan_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "message": e })),
            )
        })?;

    // Fetch the stored analysis record to return full detail
    let analysis = sqlx::query_scalar::<_, Value>(
        r#"
        SELECT row_to_json(t) FROM (
            SELECT id, zero_deposit_id,
                   avg_monthly_credit::FLOAT8,
                   avg_monthly_debit::FLOAT8,
                   net_monthly_cashflow::FLOAT8,
                   salary_credit_count,
                   irregular_credit_flag,
                   bounced_cheque_count,
                   high_debt_load_flag,
                   debt_to_income_ratio::FLOAT8,
                   cash_flow_stability_score::FLOAT8,
                   flags, statement_url, analysis_version, analyzed_at
            FROM bank_statement_analyses
            WHERE zero_deposit_id = $1
            ORDER BY analyzed_at DESC
            LIMIT 1
        ) t
        "#,
    )
    .bind(loan_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": e.to_string() })),
        )
    })?;

    Ok(Json(json!({
        "success": true,
        "message": "Bank statement analysis complete. Risk score updated.",
        "data": {
            "analysis": analysis,
            "risk_score": score
        }
    })))
}

// ─── Issue 95: Approve with decision_reasons ────────────────────────────────

/// POST /api/admin/loans/{loan_id}/approve
///
/// Extends the standard approve action with:
///   - Mandatory risk score computation (auto-scored if not yet scored)
///   - `decision_reasons` stored on the zero_deposit row
///   - Admin audit log entry carrying the full reasons payload
#[derive(serde::Deserialize)]
pub struct ApproveLoanRequest {
    pub approved_deposit_amount: f64,
    pub remarks: Option<String>,
    /// Explicit decision reasons. If omitted, auto-derived from the risk score.
    #[serde(default)]
    pub decision_reasons: Vec<DecisionReason>,
}

pub async fn approve_loan_with_reasons(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(loan_id): Path<Uuid>,
    Json(payload): Json<ApproveLoanRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut tx = state.db.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    // 1. Resolve decision reasons (explicit > auto-derived)
    let final_reasons: Vec<DecisionReason> = if payload.decision_reasons.is_empty() {
        loan_risk_service::auto_decision_reasons_from_risk(&state.db, loan_id, true).await
    } else {
        payload.decision_reasons.clone()
    };

    let reasons_json = serde_json::to_value(&final_reasons)
        .unwrap_or(json!([]));

    // 2. Apply approve + store decision_reasons
    let res = sqlx::query(
        r#"
        UPDATE zero_deposits
        SET status                  = 'approved',
            approved_deposit_amount = $1,
            admin_remarks           = $2,
            decision_reasons        = $3,
            decided_by              = $4,
            decided_at              = NOW(),
            updated_at              = NOW()
        WHERE id = $5
        "#,
    )
    .bind(payload.approved_deposit_amount)
    .bind(&payload.remarks)
    .bind(&reasons_json)
    .bind(&admin_claims.sub)
    .bind(loan_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    if res.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "Application not found" })),
        ));
    }

    // 3. Audit log
    let _ = log_admin_action(
        &state.db,
        &admin_claims.sub,
        "APPROVE_LOAN_APPLICATION",
        "LOAN",
        Some(loan_id),
        Some(json!({
            "approved_deposit_amount": payload.approved_deposit_amount,
            "remarks": payload.remarks,
            "decision_reasons": final_reasons
        })),
    )
    .await;

    tx.commit().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    Ok(Json(json!({
        "success": true,
        "message": "Loan application approved",
        "decision_reasons": final_reasons
    })))
}

// ─── Issue 95: Reject with decision_reasons ─────────────────────────────────

#[derive(serde::Deserialize)]
pub struct RejectLoanRequest {
    pub reason: String,
    /// Explicit decision reasons. If omitted, auto-derived from the risk score.
    #[serde(default)]
    pub decision_reasons: Vec<DecisionReason>,
}

pub async fn reject_loan_with_reasons(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(loan_id): Path<Uuid>,
    Json(payload): Json<RejectLoanRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut tx = state.db.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    // 1. Resolve decision reasons
    let final_reasons: Vec<DecisionReason> = if payload.decision_reasons.is_empty() {
        loan_risk_service::auto_decision_reasons_from_risk(&state.db, loan_id, false).await
    } else {
        payload.decision_reasons.clone()
    };

    let reasons_json = serde_json::to_value(&final_reasons)
        .unwrap_or(json!([]));

    // 2. Apply rejection + store decision_reasons
    let res = sqlx::query(
        r#"
        UPDATE zero_deposits
        SET status           = 'rejected',
            admin_remarks    = $1,
            decision_reasons = $2,
            decided_by       = $3,
            decided_at       = NOW(),
            updated_at       = NOW()
        WHERE id = $4
        "#,
    )
    .bind(&payload.reason)
    .bind(&reasons_json)
    .bind(&admin_claims.sub)
    .bind(loan_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    if res.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "Application not found" })),
        ));
    }

    // 3. Audit log
    let _ = log_admin_action(
        &state.db,
        &admin_claims.sub,
        "REJECT_LOAN_APPLICATION",
        "LOAN",
        Some(loan_id),
        Some(json!({
            "reason": payload.reason,
            "decision_reasons": final_reasons
        })),
    )
    .await;

    tx.commit().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    Ok(Json(json!({
        "success": true,
        "message": "Loan application rejected",
        "decision_reasons": final_reasons
    })))
}

// ─── Issue 95: GET decision reasons for a loan ──────────────────────────────

/// Admin endpoint: fetch the stored decision_reasons for a specific application.
pub async fn get_loan_decision_reasons(
    State(state): State<AppState>,
    Path(loan_id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let row = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT row_to_json(t) FROM (
            SELECT status, decision_reasons, decided_by, decided_at, admin_remarks
            FROM zero_deposits
            WHERE id = $1
        ) t
        "#,
    )
    .bind(loan_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": e.to_string() })),
        )
    })?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "message": "Application not found" })),
        )
    })?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "loan_id": loan_id,
            "status": row.get("status"),
            "decision_reasons": row.get("decision_reasons"),
            "decided_by": row.get("decided_by"),
            "decided_at": row.get("decided_at"),
            "admin_remarks": row.get("admin_remarks")
        }
    })))
}
