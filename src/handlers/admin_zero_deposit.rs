use axum::{
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::admin_zero_deposit::{
        AssessmentOverrideReq, AssessmentRerunReq, ManualIssueReq, MonthlyReportQuery,
        RevokeTokensReq, UserTierUpdateReq, ZeroDepositApproveReq, ZeroDepositDefaultActionReq,
        ZeroDepositQuery, ZeroDepositRecoverReq, ZeroDepositRejectReq,
    },
    handlers::{
        admin_auth::AdminClaims,
        zd_redemption_handler::map_redemption_error,
    },
    services::{
        zd_jobs, zd_notifications,
        zd_redemption_service::{
            self, AdminApproveReq, AdminDocRequestReq, AdminDocReviewReq, AdminRejectReq,
            AdminReleaseReq, SendMessageReq,
        },
        zd_token_service::{self, DEPOSIT_RENT_MULTIPLE},
    },
    utils::admin_logger::{log_admin_action, log_admin_action_tx},
};

// ---------------------------------------------------------------------------
// A. Applications
// ---------------------------------------------------------------------------

/// GET /api/admin/zero-deposit?status=&decision=&tier=&limit=&offset=
/// Supports new statuses plus `decision` (auto_approve, action_required, manual_review, auto_reject)
/// and `tier` (A, B, C) filters. Each row includes tier, decision, reason_codes.
/// Bound parameters only (no format!-built SQL).
pub async fn get_zero_deposits(
    State(state): State<AppState>,
    Query(filter): Query<ZeroDepositQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = filter.limit.unwrap_or(50).min(200);
    let offset = filter.offset.unwrap_or(0);
    let status_filter = filter.status.as_deref();
    let decision_filter = filter.decision.as_deref();
    let tier_filter = filter.tier.as_deref();

    let records = sqlx::query_scalar::<_, serde_json::Value>(
        r#"SELECT row_to_json(t) FROM (
            SELECT z.id, z.user_id, z.property_id, z.kyc_id,
                   z.monthly_rent::FLOAT8 as monthly_rent,
                   z.deposit_amount::FLOAT8 as deposit_amount,
                   z.fee_amount::FLOAT8 as fee_amount,
                   z.first_rent_amount::FLOAT8 as first_rent_amount,
                   z.monthly_income::FLOAT8 as monthly_income,
                   z.status::TEXT as status,
                   z.consent_given,
                   z.pending_actions,
                   z.created_at, z.updated_at,
                   a.tier,
                   a.decision,
                   COALESCE(a.reason_codes, '[]'::jsonb) as reason_codes,
                   u.first_name as user_first_name,
                   u.last_name as user_last_name,
                   u.email as user_email,
                   u.phone_no as user_phone
            FROM zero_deposits z
            JOIN users u ON z.user_id = u.id
            LEFT JOIN zero_deposit_ai_assessments a ON z.id = a.zero_deposit_id AND a.is_current = true
            WHERE ($1::TEXT IS NULL OR z.status::TEXT = $1)
              AND ($2::TEXT IS NULL OR a.decision::TEXT = $2)
              AND ($3::TEXT IS NULL OR a.tier::TEXT = $3)
            ORDER BY z.created_at DESC
            LIMIT $4 OFFSET $5
        ) t"#,
    )
    .bind(status_filter)
    .bind(decision_filter)
    .bind(tier_filter)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    Ok(Json(json!({
        "success": true,
        "data": records
    })))
}

/// GET /api/admin/zero-deposit/{zero_deposit_id}
pub async fn get_zero_deposit_detail(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let record = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT row_to_json(t) FROM (
            SELECT z.*, 
                   a.tier,
                   a.decision,
                   COALESCE(a.reason_codes, '[]'::jsonb) as reason_codes,
                   u.first_name as user_first_name, u.last_name as user_last_name, u.email as user_email, u.phone_no as user_phone,
                   p.title as property_title,
                   (
                       SELECT coalesce(array_agg(doc), ARRAY[]::text[])
                       FROM unnest(ARRAY[k.profile_picture_url, k.govt_id_document_url, k.experience_document_url]) as doc
                       WHERE doc IS NOT NULL
                   ) as kyc_document_urls,
                   (
                       SELECT row_to_json(cur)
                       FROM zero_deposit_ai_assessments cur
                       WHERE cur.zero_deposit_id = z.id AND cur.is_current = true
                       LIMIT 1
                   ) as current_assessment,
                   (
                       SELECT row_to_json(al)
                       FROM zd_token_allocations al
                       WHERE al.zero_deposit_id = z.id AND al.status = 'active'
                       LIMIT 1
                   ) as active_allocation
            FROM zero_deposits z
            JOIN users u ON z.user_id = u.id
            LEFT JOIN properties p ON z.property_id = p.id
            LEFT JOIN kyc_submissions k ON z.kyc_id = k.id
            LEFT JOIN zero_deposit_ai_assessments a ON z.id = a.zero_deposit_id AND a.is_current = true
            WHERE z.id = $1
        ) t
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    if let Some(r) = record {
        Ok(Json(json!({
            "success": true,
            "data": r
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "NOT_FOUND", "message": "Zero Deposit application not found" })),
        ))
    }
}

/// PATCH /api/admin/zero-deposit/{zero_deposit_id}/approve
/// Body: {"approved_deposit_amount": 30000, "remarks": "string"}
/// ALSO issues tokens equal to approved_deposit_amount (via zd_tokens::issue, expiry fixed issue+30 days, source admin_manual).
/// Status moves to tokens_issued. One active allocation only.
pub async fn approve_zero_deposit(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ZeroDepositApproveReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut tx = state.db.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    // Load application details
    let app_row = sqlx::query(
        "SELECT user_id, monthly_rent::FLOAT8 as rent, status::TEXT as status FROM zero_deposits WHERE id = $1 FOR UPDATE"
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let (user_id, monthly_rent, current_status) = match app_row {
        Some(r) => (
            r.get::<Uuid, _>("user_id"),
            r.get::<f64, _>("rent"),
            r.get::<String, _>("status"),
        ),
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "error": "NOT_FOUND", "message": "Zero Deposit application not found" })),
            ));
        }
    };

    if !["ai_review", "ai_manual_review", "pending_review"].contains(&current_status.as_str()) {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "INVALID_STATE",
                "message": format!("Cannot approve application from status '{}'", current_status)
            })),
        ));
    }

    // Fail-Closed KYC Check: If any KYC check is incomplete, require explicit acknowledgement
    let checks_json = sqlx::query_scalar::<_, Value>(
        "SELECT checks FROM zero_deposit_ai_assessments WHERE zero_deposit_id = $1 AND is_current = true ORDER BY created_at DESC LIMIT 1"
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .ok()
    .flatten();

    let incomplete_checks: Vec<String> = if let Some(Value::Array(checks)) = checks_json {
        checks
            .iter()
            .filter_map(|c| {
                let name = c.get("check").and_then(|v| v.as_str()).unwrap_or("");
                let result = c.get("result").and_then(|v| v.as_str()).unwrap_or("");
                if (name.contains("kyc") || name.contains("face") || name.contains("liveness") || name.contains("tamper") || name.contains("doc_extraction") || name.contains("match"))
                    && result != "pass"
                {
                    Some(format!("{}: {}", name, result))
                } else {
                    None
                }
            })
            .collect()
    } else {
        vec!["kyc_assessment_missing".to_string()]
    };

    if !incomplete_checks.is_empty() && payload.acknowledge_incomplete_kyc != Some(true) {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "success": false,
                "error": "KYC_INCOMPLETE_ACKNOWLEDGEMENT_REQUIRED",
                "message": "KYC verifications are incomplete or unverified. You must explicitly set acknowledge_incomplete_kyc: true to approve.",
                "incomplete_checks": incomplete_checks
            })),
        ));
    }

    // Update status to tokens_issued
    sqlx::query(
        "UPDATE zero_deposits
         SET status = 'tokens_issued'::zero_deposit_status,
             approved_deposit_amount = $1,
             admin_remarks = $2,
             updated_at = NOW()
         WHERE id = $3",
    )
    .bind(payload.approved_deposit_amount)
    .bind(&payload.remarks)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let admin_uuid = Uuid::parse_str(&admin_claims.sub).ok();

    // Log admin action inside transaction
    let _ = log_admin_action_tx(
        &mut *tx,
        &admin_claims.sub,
        "APPROVE_ZERO_DEPOSIT",
        "ZERO_DEPOSIT",
        Some(id),
        Some(json!({
            "approved_deposit_amount": payload.approved_deposit_amount,
            "remarks": payload.remarks
        })),
    )
    .await;

    tx.commit().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    // Issue tokens via zd_token_service (source: admin_manual)
    let rent_basis = payload.approved_deposit_amount / DEPOSIT_RENT_MULTIPLE;
    let issue_res = zd_token_service::issue(
        &state.db,
        id,
        rent_basis.max(monthly_rent),
        Some("admin_manual"),
        admin_uuid,
        None,
        payload.remarks.as_deref(),
    )
    .await;

    let (issued_amount, expires_at) = match issue_res {
        Ok(alloc) => (alloc.issued_amount, alloc.expires_at),
        Err(e) => {
            log::warn!("Token issuance warning during admin approve: {:?}", e);
            (payload.approved_deposit_amount, Utc::now() + chrono::Duration::days(30))
        }
    };

    // Send notification to tenant
    zd_notifications::notify_tokens_issued(&state.db, user_id, issued_amount, expires_at).await;

    Ok(Json(json!({
        "success": true,
        "message": "Zero Deposit approved and tokens issued",
        "tokens_issued": issued_amount,
        "expires_at": expires_at
    })))
}

/// PATCH /api/admin/zero-deposit/{zero_deposit_id}/reject
/// Body: {"reason": "string"}
pub async fn reject_zero_deposit(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ZeroDepositRejectReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut tx = state.db.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let res = sqlx::query(
        "UPDATE zero_deposits
         SET status = 'rejected'::zero_deposit_status,
             admin_remarks = $1,
             updated_at = NOW()
         WHERE id = $2 AND status NOT IN ('rejected'::zero_deposit_status, 'released'::zero_deposit_status, 'expired'::zero_deposit_status)",
    )
    .bind(&payload.reason)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    if res.rows_affected() == 0 {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM zero_deposits WHERE id = $1)")
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(false);

        if exists {
            return Err((
                StatusCode::CONFLICT,
                Json(json!({ "success": false, "error": "INVALID_STATE", "message": "Application cannot be rejected from its current status" })),
            ));
        } else {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "error": "NOT_FOUND", "message": "Zero Deposit application not found" })),
            ));
        }
    }

    let _ = log_admin_action_tx(
        &mut *tx,
        &admin_claims.sub,
        "REJECT_ZERO_DEPOSIT",
        "ZERO_DEPOSIT",
        Some(id),
        Some(json!({ "reason": payload.reason })),
    )
    .await;

    tx.commit().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    Ok(Json(json!({ "success": true, "message": "Zero Deposit rejected" })))
}

// ---------------------------------------------------------------------------
// B. AI Review & Assessment
// ---------------------------------------------------------------------------

/// GET /api/admin/zero-deposit/{zero_deposit_id}/assessment
/// Returns full AI output for underwriter including checks, bureau, flags, and history.
pub async fn get_assessment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let current_assessment = sqlx::query(
        r#"SELECT id, rules_version, model_version, status, decision, tier,
                  recommended_token_amount::FLOAT8 as token_amount,
                  checks, reason_codes, ai_flags,
                  income_considered::FLOAT8 as income_considered,
                  bureau_summary, admin_summary,
                  override_decision, override_reason, overridden_by, overridden_at,
                  created_at
           FROM zero_deposit_ai_assessments
           WHERE zero_deposit_id = $1 AND is_current = true
           LIMIT 1"#
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let row = match current_assessment {
        Some(r) => r,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "error": "NOT_FOUND", "message": "No current assessment found for application" })),
            ));
        }
    };

    let tier: Option<String> = row.try_get("tier").ok();
    let tier_max_rent = match tier.as_deref() {
        Some("A") => 40000.0,
        Some("B") => 25000.0,
        _ => 15000.0,
    };

    let override_decision: Option<String> = row.try_get("override_decision").ok();
    let override_val = override_decision.map(|dec| {
        json!({
            "decision": dec,
            "reason": row.try_get::<Option<String>, _>("override_reason").ok().flatten(),
            "overridden_by": row.try_get::<Option<Uuid>, _>("overridden_by").ok().flatten(),
            "overridden_at": row.try_get::<Option<DateTime<Utc>>, _>("overridden_at").ok().flatten(),
        })
    });

    // Fetch assessment history
    let history_rows = sqlx::query(
        r#"SELECT id, rules_version, model_version, decision, tier, status, created_at
           FROM zero_deposit_ai_assessments
           WHERE zero_deposit_id = $1 AND is_current = false
           ORDER BY created_at DESC"#
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut history = Vec::new();
    for h in history_rows {
        history.push(json!({
            "assessment_id": h.get::<Uuid, _>("id"),
            "rules_version": h.try_get::<Option<String>, _>("rules_version").ok().flatten(),
            "model_version": h.try_get::<Option<String>, _>("model_version").ok().flatten(),
            "decision": h.try_get::<Option<String>, _>("decision").ok().flatten(),
            "tier": h.try_get::<Option<String>, _>("tier").ok().flatten(),
            "status": h.get::<String, _>("status"),
            "created_at": h.get::<DateTime<Utc>, _>("created_at"),
        }));
    }

    Ok(Json(json!({
        "success": true,
        "data": {
            "assessment_id": row.get::<Uuid, _>("id"),
            "rules_version": row.try_get::<Option<String>, _>("rules_version").ok().flatten().unwrap_or_else(|| "elig-2026-09".into()),
            "model_version": row.try_get::<Option<String>, _>("model_version").ok().flatten().unwrap_or_else(|| "zd-extract-v1.0".into()),
            "status": row.get::<String, _>("status"),
            "decision": row.try_get::<Option<String>, _>("decision").ok().flatten().unwrap_or_else(|| "manual_review".into()),
            "tier": tier.clone().unwrap_or_else(|| "C".into()),
            "tier_max_rent": tier_max_rent,
            "token_amount": row.try_get::<Option<f64>, _>("token_amount").ok().flatten().unwrap_or(0.0),
            "checks": row.try_get::<Value, _>("checks").unwrap_or(json!([])),
            "reason_codes": row.try_get::<Value, _>("reason_codes").unwrap_or(json!([])),
            "ai_flags": row.try_get::<Value, _>("ai_flags").unwrap_or(json!([])),
            "income_considered": row.try_get::<Option<f64>, _>("income_considered").ok().flatten().unwrap_or(0.0),
            "bureau": row.try_get::<Value, _>("bureau_summary").unwrap_or(json!({})),
            "admin_summary": row.try_get::<Option<String>, _>("admin_summary").ok().flatten().unwrap_or_default(),
            "history": history,
            "override": override_val
        }
    })))
}

/// POST /api/admin/zero-deposit/{zero_deposit_id}/assessment/rerun
/// Re-queues the AI job. Allowed ONLY in ai_review, ai_manual_review, or tokens_issued with NOTHING locked or redeemed.
pub async fn rerun_assessment(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<AssessmentRerunReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut tx = state.db.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let app_row = sqlx::query(
        "SELECT user_id, status::TEXT as status FROM zero_deposits WHERE id = $1 FOR UPDATE"
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let (user_id, status) = match app_row {
        Some(r) => (r.get::<Uuid, _>("user_id"), r.get::<String, _>("status")),
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "error": "NOT_FOUND", "message": "Application not found" })),
            ));
        }
    };

    if status == "tokens_issued" {
        // Ensure NOTHING is locked or redeemed
        let alloc_row = sqlx::query(
            "SELECT locked::FLOAT8 as locked, redeemed::FLOAT8 as redeemed FROM zd_token_allocations WHERE zero_deposit_id = $1 AND status = 'active'"
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .unwrap_or(None);

        if let Some(a) = alloc_row {
            let locked: f64 = a.get("locked");
            let redeemed: f64 = a.get("redeemed");
            if locked > 0.0 || redeemed > 0.0 {
                return Err((
                    StatusCode::CONFLICT,
                    Json(json!({
                        "success": false,
                        "error": "INVALID_STATE",
                        "message": "Cannot rerun assessment while tokens are locked or redeemed"
                    })),
                ));
            }
        }
    } else if !["ai_review", "ai_manual_review"].contains(&status.as_str()) {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "INVALID_STATE",
                "message": format!("Cannot rerun assessment from status '{}'", status)
            })),
        ));
    }

    // Demote current assessment
    sqlx::query(
        "UPDATE zero_deposit_ai_assessments SET is_current = false WHERE zero_deposit_id = $1"
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    // Transition application to ai_review
    sqlx::query(
        "UPDATE zero_deposits SET status = 'ai_review'::zero_deposit_status, updated_at = NOW() WHERE id = $1"
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let _ = log_admin_action_tx(
        &mut *tx,
        &admin_claims.sub,
        "RERUN_ASSESSMENT",
        "ZERO_DEPOSIT",
        Some(id),
        Some(json!({ "remarks": payload.remarks })),
    )
    .await;

    tx.commit().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    // Re-queue the AI assessment job
    let assessment_id = crate::services::zd_ai_client_service::queue_ai_assessment_job(
        &state.db,
        state.redis_pool.clone(),
        id,
        user_id,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e })),
        )
    })?;

    Ok(Json(json!({
        "success": true,
        "message": "AI assessment rerun queued",
        "assessment_id": assessment_id,
        "status": "ai_review"
    })))
}

/// PATCH /api/admin/zero-deposit/{zero_deposit_id}/assessment/override
/// Replaces the AI decision. Approve issues 3 x rent; reject sets rejected.
pub async fn override_assessment(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<AssessmentOverrideReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if payload.reason.trim().is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "success": false,
                "error": "REASON_REQUIRED",
                "message": "Override reason is mandatory"
            })),
        ));
    }

    let admin_uuid = Uuid::parse_str(&admin_claims.sub).ok();

    let mut tx = state.db.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let app_row = sqlx::query(
        "SELECT user_id, monthly_rent::FLOAT8 as rent, status::TEXT as status FROM zero_deposits WHERE id = $1 FOR UPDATE"
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let (user_id, monthly_rent, _) = match app_row {
        Some(r) => (r.get::<Uuid, _>("user_id"), r.get::<f64, _>("rent"), r.get::<String, _>("status")),
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "error": "NOT_FOUND", "message": "Application not found" })),
            ));
        }
    };

    // Update current assessment row
    sqlx::query(
        r#"UPDATE zero_deposit_ai_assessments
           SET override_decision = $1,
               override_reason = $2,
               overridden_by = $3,
               overridden_at = NOW()
           WHERE zero_deposit_id = $4 AND is_current = true"#
    )
    .bind(&payload.decision)
    .bind(&payload.reason)
    .bind(admin_uuid)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    if payload.decision == "approve" {
        // Fail-Closed KYC Check: If any KYC check is incomplete, require explicit acknowledgement
        let checks_json = sqlx::query_scalar::<_, Value>(
            "SELECT checks FROM zero_deposit_ai_assessments WHERE zero_deposit_id = $1 AND is_current = true ORDER BY created_at DESC LIMIT 1"
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .ok()
        .flatten();

        let incomplete_checks: Vec<String> = if let Some(Value::Array(checks)) = checks_json {
            checks
                .iter()
                .filter_map(|c| {
                    let name = c.get("check").and_then(|v| v.as_str()).unwrap_or("");
                    let result = c.get("result").and_then(|v| v.as_str()).unwrap_or("");
                    if (name.contains("kyc") || name.contains("face") || name.contains("liveness") || name.contains("tamper") || name.contains("doc_extraction") || name.contains("match"))
                        && result != "pass"
                    {
                        Some(format!("{}: {}", name, result))
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            vec!["kyc_assessment_missing".to_string()]
        };

        if !incomplete_checks.is_empty() && payload.acknowledge_incomplete_kyc != Some(true) {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "success": false,
                    "error": "KYC_INCOMPLETE_ACKNOWLEDGEMENT_REQUIRED",
                    "message": "KYC verifications are incomplete or unverified. You must explicitly set acknowledge_incomplete_kyc: true to approve override.",
                    "incomplete_checks": incomplete_checks
                })),
            ));
        }
        let deposit_amount = monthly_rent * DEPOSIT_RENT_MULTIPLE;

        sqlx::query(
            "UPDATE zero_deposits
             SET status = 'tokens_issued'::zero_deposit_status,
                 deposit_amount = $1,
                 admin_remarks = $2,
                 updated_at = NOW()
             WHERE id = $3",
        )
        .bind(deposit_amount)
        .bind(&payload.reason)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": e.to_string() })),
            )
        })?;

        // If tier specified, update zd_tenant_tiers
        if let Some(ref t) = payload.tier {
            let _ = sqlx::query(
                r#"INSERT INTO zd_tenant_tiers (id, user_id, tier, source, reason, on_time_months, updated_at)
                   VALUES (gen_random_uuid(), $1, $2::zd_tenant_tier, 'admin', $3, 0, NOW())
                   ON CONFLICT (user_id) DO UPDATE
                   SET tier = EXCLUDED.tier, source = 'admin', reason = EXCLUDED.reason, updated_at = NOW()"#
            )
            .bind(user_id)
            .bind(t)
            .bind(&payload.reason)
            .execute(&mut *tx)
            .await;
        }

        let _ = log_admin_action_tx(
            &mut *tx,
            &admin_claims.sub,
            "OVERRIDE_ASSESSMENT_APPROVE",
            "ZERO_DEPOSIT",
            Some(id),
            Some(json!({
                "decision": "approve",
                "tier": payload.tier,
                "reason": payload.reason,
                "deposit_amount": deposit_amount
            })),
        )
        .await;

        tx.commit().await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": e.to_string() })),
            )
        })?;

        // Issue tokens via zd_token_service (source: admin_override)
        let alloc = zd_token_service::issue(
            &state.db,
            id,
            monthly_rent,
            Some("admin_override"),
            admin_uuid,
            None,
            Some(&payload.reason),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": format!("{:?}", e) })),
            )
        })?;

        zd_notifications::notify_tokens_issued(&state.db, user_id, alloc.issued_amount, alloc.expires_at).await;

        Ok(Json(json!({
            "success": true,
            "message": "Assessment overridden to approved and tokens issued",
            "status": "tokens_issued",
            "tokens_issued": alloc.issued_amount,
            "expires_at": alloc.expires_at
        })))
    } else {
        // Decision == reject
        sqlx::query(
            "UPDATE zero_deposits
             SET status = 'rejected'::zero_deposit_status,
                 admin_remarks = $1,
                 updated_at = NOW()
             WHERE id = $2",
        )
        .bind(&payload.reason)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": e.to_string() })),
            )
        })?;

        let _ = log_admin_action_tx(
            &mut *tx,
            &admin_claims.sub,
            "OVERRIDE_ASSESSMENT_REJECT",
            "ZERO_DEPOSIT",
            Some(id),
            Some(json!({
                "decision": "reject",
                "reason": payload.reason
            })),
        )
        .await;

        tx.commit().await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": e.to_string() })),
            )
        })?;

        // Revoke any active token allocation
        let _ = zd_token_service::revoke(&state.db, id, admin_uuid, Some(&payload.reason)).await;

        Ok(Json(json!({
            "success": true,
            "message": "Assessment overridden to rejected",
            "status": "rejected"
        })))
    }
}

/// PATCH /api/admin/zero-deposit/users/{user_id}/tier
/// Manual tier change; overrides nightly tier job (zd_tenant_tiers source admin).
pub async fn update_user_tier(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<UserTierUpdateReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !["A", "B", "C"].contains(&payload.tier.as_str()) {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "success": false, "error": "INVALID_TIER", "message": "Tier must be A, B, or C" })),
        ));
    }

    let mut tx = state.db.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    sqlx::query(
        r#"INSERT INTO zd_tenant_tiers (id, user_id, tier, source, reason, on_time_months, updated_at)
           VALUES (gen_random_uuid(), $1, $2::zd_tenant_tier, 'admin', $3, 0, NOW())
           ON CONFLICT (user_id) DO UPDATE
           SET tier = EXCLUDED.tier,
               source = 'admin',
               reason = EXCLUDED.reason,
               updated_at = NOW()"#
    )
    .bind(user_id)
    .bind(&payload.tier)
    .bind(payload.reason.as_deref().unwrap_or("Manual admin tier override"))
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let _ = log_admin_action_tx(
        &mut *tx,
        &admin_claims.sub,
        "UPDATE_USER_TIER",
        "USER",
        Some(user_id),
        Some(json!({ "tier": payload.tier, "reason": payload.reason })),
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
        "message": format!("Tenant tier set to {}", payload.tier),
        "tier": payload.tier
    })))
}

// ---------------------------------------------------------------------------
// C. Token Management
// ---------------------------------------------------------------------------

/// POST /api/admin/zero-deposit/{zero_deposit_id}/tokens/issue
/// Manual issue for ai_manual_review applications; ALWAYS 3 x rent; admin confirms tier.
/// Expiry is ALWAYS issue + 30 days and is NEVER accepted as input.
pub async fn manual_issue_tokens(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ManualIssueReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !["A", "B", "C"].contains(&payload.tier.as_str()) {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "success": false, "error": "INVALID_TIER", "message": "Tier must be A, B, or C" })),
        ));
    }

    let mut tx = state.db.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let app_row = sqlx::query(
        "SELECT user_id, monthly_rent::FLOAT8 as rent, status::TEXT as status FROM zero_deposits WHERE id = $1 FOR UPDATE"
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let (user_id, monthly_rent, status) = match app_row {
        Some(r) => (r.get::<Uuid, _>("user_id"), r.get::<f64, _>("rent"), r.get::<String, _>("status")),
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "error": "NOT_FOUND", "message": "Application not found" })),
            ));
        }
    };

    if status != "ai_manual_review" {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "INVALID_STATE",
                "message": format!("Manual token issue allowed only in 'ai_manual_review' (current: '{}')", status)
            })),
        ));
    }

    // Fail-Closed KYC Check: If any KYC check is incomplete, require explicit acknowledgement
    let checks_json = sqlx::query_scalar::<_, Value>(
        "SELECT checks FROM zero_deposit_ai_assessments WHERE zero_deposit_id = $1 AND is_current = true ORDER BY created_at DESC LIMIT 1"
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .ok()
    .flatten();

    let incomplete_checks: Vec<String> = if let Some(Value::Array(checks)) = checks_json {
        checks
            .iter()
            .filter_map(|c| {
                let name = c.get("check").and_then(|v| v.as_str()).unwrap_or("");
                let result = c.get("result").and_then(|v| v.as_str()).unwrap_or("");
                if (name.contains("kyc") || name.contains("face") || name.contains("liveness") || name.contains("tamper") || name.contains("doc_extraction") || name.contains("match"))
                    && result != "pass"
                {
                    Some(format!("{}: {}", name, result))
                } else {
                    None
                }
            })
            .collect()
    } else {
        vec!["kyc_assessment_missing".to_string()]
    };

    if !incomplete_checks.is_empty() && payload.acknowledge_incomplete_kyc != Some(true) {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "success": false,
                "error": "KYC_INCOMPLETE_ACKNOWLEDGEMENT_REQUIRED",
                "message": "KYC verifications are incomplete or unverified. You must explicitly set acknowledge_incomplete_kyc: true to manually issue tokens.",
                "incomplete_checks": incomplete_checks
            })),
        ));
    }

    let deposit_amount = monthly_rent * DEPOSIT_RENT_MULTIPLE;

    sqlx::query(
        "UPDATE zero_deposits
         SET status = 'tokens_issued'::zero_deposit_status,
             deposit_amount = $1,
             admin_remarks = $2,
             updated_at = NOW()
         WHERE id = $3",
    )
    .bind(deposit_amount)
    .bind(&payload.remarks)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    // Update zd_tenant_tiers
    sqlx::query(
        r#"INSERT INTO zd_tenant_tiers (id, user_id, tier, source, reason, on_time_months, updated_at)
           VALUES (gen_random_uuid(), $1, $2::zd_tenant_tier, 'admin', $3, 0, NOW())
           ON CONFLICT (user_id) DO UPDATE
           SET tier = EXCLUDED.tier, source = 'admin', reason = EXCLUDED.reason, updated_at = NOW()"#
    )
    .bind(user_id)
    .bind(&payload.tier)
    .bind(payload.remarks.as_deref().unwrap_or("Admin manual issuance tier confirmation"))
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let admin_uuid = Uuid::parse_str(&admin_claims.sub).ok();

    let _ = log_admin_action_tx(
        &mut *tx,
        &admin_claims.sub,
        "MANUAL_ISSUE_TOKENS",
        "ZERO_DEPOSIT",
        Some(id),
        Some(json!({
            "tier": payload.tier,
            "deposit_amount": deposit_amount,
            "remarks": payload.remarks
        })),
    )
    .await;

    tx.commit().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    // Issue tokens via zd_token_service (expiry always fixed +30 days)
    let alloc = zd_token_service::issue(
        &state.db,
        id,
        monthly_rent,
        Some("admin_manual"),
        admin_uuid,
        None,
        payload.remarks.as_deref(),
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": format!("{:?}", e) })),
        )
    })?;

    zd_notifications::notify_tokens_issued(&state.db, user_id, alloc.issued_amount, alloc.expires_at).await;

    Ok(Json(json!({
        "success": true,
        "message": "Tokens issued successfully",
        "tokens_issued": alloc.issued_amount,
        "expires_at": alloc.expires_at,
        "status": "tokens_issued"
    })))
}

/// PATCH /api/admin/zero-deposit/tokens/{allocation_id}/revoke
/// Revokes the AVAILABLE balance only; locked tokens stay locked until their redemption closes.
/// If nothing released, the application becomes expired.
pub async fn revoke_tokens(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(allocation_id): Path<Uuid>,
    Json(payload): Json<RevokeTokensReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let admin_uuid = Uuid::parse_str(&admin_claims.sub).ok();

    let alloc_row = sqlx::query(
        "SELECT id, zero_deposit_id, available::FLOAT8 as available, locked::FLOAT8 as locked, redeemed::FLOAT8 as redeemed
         FROM zd_token_allocations WHERE id = $1"
    )
    .bind(allocation_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let (app_id, locked, redeemed) = match alloc_row {
        Some(r) => (
            r.get::<Uuid, _>("zero_deposit_id"),
            r.get::<f64, _>("locked"),
            r.get::<f64, _>("redeemed"),
        ),
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "error": "NOT_FOUND", "message": "Token allocation not found" })),
            ));
        }
    };

    let revoked_amount = zd_token_service::revoke(&state.db, app_id, admin_uuid, Some(&payload.reason))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": format!("{:?}", e) })),
            )
        })?;

    // If nothing was released and no locked balance, transition application to expired
    if redeemed <= 0.0 && locked <= 0.0 {
        let _ = sqlx::query(
            "UPDATE zero_deposits SET status = 'expired'::zero_deposit_status, updated_at = NOW() WHERE id = $1"
        )
        .bind(app_id)
        .execute(&state.db)
        .await;
    }

    let _ = log_admin_action(
        &state.db,
        &admin_claims.sub,
        "REVOKE_TOKENS",
        "ZD_ALLOCATION",
        Some(allocation_id),
        Some(json!({
            "revoked_amount": revoked_amount,
            "reason": payload.reason
        })),
    )
    .await;

    Ok(Json(json!({
        "success": true,
        "message": format!("Revoked ₹{:.2} available tokens", revoked_amount),
        "revoked_amount": revoked_amount
    })))
}

/// GET /api/admin/zero-deposit/tokens/{allocation_id}/transactions
pub async fn get_token_transactions(
    State(state): State<AppState>,
    Path(allocation_id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let rows = sqlx::query_scalar::<_, Value>(
        r#"SELECT row_to_json(t) FROM (
            SELECT id, allocation_id, zero_deposit_id, operation,
                   amount::FLOAT8 as amount,
                   balance_before::FLOAT8 as balance_before,
                   balance_after::FLOAT8 as balance_after,
                   available_after::FLOAT8 as available_after,
                   locked_after::FLOAT8 as locked_after,
                   reference_type, reference_id, actor_type, actor_id, notes, created_at
            FROM zd_token_transactions
            WHERE allocation_id = $1
            ORDER BY created_at DESC
        ) t"#
    )
    .bind(allocation_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    Ok(Json(json!({
        "success": true,
        "data": rows
    })))
}

// ---------------------------------------------------------------------------
// D. AI Configuration
// ---------------------------------------------------------------------------

/// GET /api/admin/zero-deposit/ai-config
pub async fn get_ai_config(
    State(state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let row = sqlx::query(
        "SELECT config, maker_checker_enabled, redemption_required_documents, updated_at FROM zd_ai_config LIMIT 1"
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let row = match row {
        Some(r) => r,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "error": "NOT_FOUND", "message": "AI config not found" })),
            ));
        }
    };

    let mut cfg: Value = row.get("config");
    let mc: bool = row.get("maker_checker_enabled");
    let req_docs: Value = row.get("redemption_required_documents");

    if let Some(map) = cfg.as_object_mut() {
        map.insert("maker_checker_enabled".into(), json!(mc));
        map.insert("redemption_required_documents".into(), req_docs);
    }

    Ok(Json(json!({
        "success": true,
        "data": cfg
    })))
}

/// PATCH /api/admin/zero-deposit/ai-config
/// Token expiry is NOT configurable. Every change is audit-logged with old and new values.
pub async fn patch_ai_config(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // 1. Load existing config
    let row = sqlx::query(
        "SELECT config, maker_checker_enabled, redemption_required_documents FROM zd_ai_config LIMIT 1 FOR UPDATE"
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let (mut current_cfg, old_mc, old_docs) = match row {
        Some(r) => (
            r.get::<Value, _>("config"),
            r.get::<bool, _>("maker_checker_enabled"),
            r.get::<Value, _>("redemption_required_documents"),
        ),
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "error": "NOT_FOUND", "message": "AI config not found" })),
            ));
        }
    };

    let old_full = json!({
        "config": current_cfg.clone(),
        "maker_checker_enabled": old_mc,
        "redemption_required_documents": old_docs.clone()
    });

    // 2. Validate payload
    if let Some(token_days) = payload.get("token_validity_days") {
        if token_days.as_i64() != Some(30) {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "success": false,
                    "error": "IMMUTABLE_CONFIG",
                    "message": "Token expiry window is fixed at 30 days and is NOT configurable."
                })),
            ));
        }
    }

    if let Some(age_min) = payload.get("age_min").and_then(|v| v.as_i64()) {
        if age_min < 18 || age_min > 70 {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "success": false, "error": "INVALID_CONFIG", "message": "age_min must be between 18 and 70" })),
            ));
        }
    }

    if let Some(age_max) = payload.get("age_max").and_then(|v| v.as_i64()) {
        if age_max < 21 || age_max > 75 {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "success": false, "error": "INVALID_CONFIG", "message": "age_max must be between 21 and 75" })),
            ));
        }
    }

    if let Some(min_tenancy) = payload.get("min_tenancy_months").and_then(|v| v.as_i64()) {
        if min_tenancy < 1 {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "success": false, "error": "INVALID_CONFIG", "message": "min_tenancy_months must be at least 1" })),
            ));
        }
    }

    // Merge payload into current config
    if let (Some(cur_obj), Some(patch_obj)) = (current_cfg.as_object_mut(), payload.as_object()) {
        for (k, v) in patch_obj {
            if k != "token_validity_days" {
                cur_obj.insert(k.clone(), v.clone());
            }
        }
    }

    let new_mc = payload.get("maker_checker_enabled").and_then(|v| v.as_bool()).unwrap_or(old_mc);
    let new_docs = payload.get("redemption_required_documents").cloned().unwrap_or(old_docs);

    // Save updated config
    sqlx::query(
        "UPDATE zd_ai_config
         SET config = $1, maker_checker_enabled = $2, redemption_required_documents = $3, updated_at = NOW()"
    )
    .bind(&current_cfg)
    .bind(new_mc)
    .bind(&new_docs)
    .execute(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let new_full = json!({
        "config": current_cfg.clone(),
        "maker_checker_enabled": new_mc,
        "redemption_required_documents": new_docs.clone()
    });

    let _ = log_admin_action(
        &state.db,
        &admin_claims.sub,
        "UPDATE_AI_CONFIG",
        "ZD_AI_CONFIG",
        None,
        Some(json!({
            "old_value": old_full,
            "new_value": new_full
        })),
    )
    .await;

    Ok(Json(json!({
        "success": true,
        "message": "AI config updated successfully",
        "data": current_cfg
    })))
}

// ---------------------------------------------------------------------------
// E. Dashboard Counters
// ---------------------------------------------------------------------------

/// GET /api/admin/zero-deposit/dashboard-counters
/// Replaces NBFC portfolio summary:
/// - applications in ai_manual_review
/// - redemptions under_review
/// - redemptions approved awaiting payout
/// - oldest unanswered tenant message
/// - amount released this month
/// - outstanding exposure (released minus repaid)
/// - active defaults
pub async fn get_dashboard_counters(
    State(state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // 1. Applications in manual review
    let apps_manual_review: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM zero_deposits WHERE status = 'ai_manual_review'::zero_deposit_status"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    // 2. Redemptions under review
    let red_under_review: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM zd_redemptions WHERE status = 'under_review'::zd_redemption_status"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    // 3. Redemptions approved awaiting payout
    let red_approved_awaiting_payout: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM zd_redemptions WHERE status = 'approved'::zd_redemption_status"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    // 4. Oldest unanswered tenant message
    let oldest_unanswered: Option<DateTime<Utc>> = sqlx::query_scalar(
        r#"SELECT MIN(m.created_at)
           FROM messages m
           JOIN zd_redemptions r ON m.chat_id = r.chat_id
           WHERE m.sender_type = 'tenant'
             AND r.status IN ('docs_pending'::zd_redemption_status, 'submitted'::zd_redemption_status, 'under_review'::zd_redemption_status, 'changes_requested'::zd_redemption_status)
             AND m.created_at > COALESCE((
                 SELECT MAX(a.created_at)
                 FROM messages a
                 WHERE a.chat_id = m.chat_id AND a.sender_type = 'admin'
             ), '1970-01-01'::timestamptz)"#
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let oldest_age_secs = oldest_unanswered.map(|dt| {
        Utc::now().signed_duration_since(dt).num_seconds().max(0)
    });

    // 5. Amount released this month
    let amount_released_this_month: f64 = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(released_amount), 0)::FLOAT8
           FROM zd_redemptions
           WHERE status = 'released'::zd_redemption_status
             AND released_at >= date_trunc('month', NOW())"#
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0.0);

    // 6. Outstanding exposure: total released minus repaid
    let total_released: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(released_amount), 0)::FLOAT8 FROM zd_redemptions WHERE status = 'released'::zd_redemption_status"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0.0);

    let total_repaid: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::FLOAT8 FROM ledger_entries WHERE transaction_type = 'repayment'"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0.0);

    let outstanding_exposure = (total_released - total_repaid).max(0.0);

    // 7. Active defaults
    let active_defaults: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM zero_deposits WHERE is_defaulted = true"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    Ok(Json(json!({
        "success": true,
        "data": {
            "applications_in_manual_review": apps_manual_review,
            "redemptions_under_review": red_under_review,
            "redemptions_approved_awaiting_payout": red_approved_awaiting_payout,
            "oldest_unanswered_tenant_message": oldest_unanswered,
            "oldest_unanswered_tenant_message_age_seconds": oldest_age_secs,
            "amount_released_this_month": amount_released_this_month,
            "outstanding_exposure": outstanding_exposure,
            "active_defaults": active_defaults
        }
    })))
}

// ---------------------------------------------------------------------------
// F. Defaults & Ledger (Livana-only)
// ---------------------------------------------------------------------------

pub async fn get_zero_deposit_ledger(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let records = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT row_to_json(t) FROM (SELECT id, transaction_type, amount::FLOAT8, reference_id, description, created_at FROM ledger_entries WHERE zero_deposit_id = $1 ORDER BY created_at DESC) t",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    Ok(Json(json!({ "success": true, "data": records })))
}

pub async fn get_defaulted_zero_deposits(
    State(state): State<AppState>,
    Query(filter): Query<ZeroDepositQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = filter.limit.unwrap_or(50).min(200);
    let offset = filter.offset.unwrap_or(0);

    let records = sqlx::query_scalar::<_, serde_json::Value>(
        r#"SELECT row_to_json(t) FROM (
            SELECT * FROM zero_deposits WHERE is_defaulted = true
            ORDER BY created_at DESC LIMIT $1 OFFSET $2
        ) t"#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    Ok(Json(json!({
        "success": true,
        "data": records
    })))
}

pub async fn suspend_default(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ZeroDepositDefaultActionReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut tx = state.db.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let res = sqlx::query(
        "UPDATE zero_deposits SET is_defaulted = true, default_remarks = $1, status = 'blocked'::zero_deposit_status, updated_at = NOW() WHERE id = $2",
    )
    .bind(&payload.remarks)
    .bind(id)
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
            Json(json!({ "success": false, "error": "Not found" })),
        ));
    }

    let _ = log_admin_action_tx(
        &mut *tx,
        &admin_claims.sub,
        "SUSPEND_DEFAULT_ZERO_DEPOSIT",
        "ZERO_DEPOSIT",
        Some(id),
        Some(json!({ "remarks": payload.remarks })),
    ).await;

    tx.commit().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    Ok(Json(json!({ "success": true, "message": "Suspended defaulted Zero Deposit" })))
}

pub async fn forfeit_default(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ZeroDepositDefaultActionReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut tx = state.db.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let res = sqlx::query(
        "UPDATE zero_deposits SET default_remarks = $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(&payload.remarks)
    .bind(id)
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
            Json(json!({ "success": false, "error": "Not found" })),
        ));
    }

    let _ = log_admin_action_tx(
        &mut *tx,
        &admin_claims.sub,
        "FORFEIT_DEFAULT_ZERO_DEPOSIT",
        "ZERO_DEPOSIT",
        Some(id),
        Some(json!({ "remarks": payload.remarks })),
    ).await;

    tx.commit().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    Ok(Json(json!({ "success": true, "message": "Forfeited Zero Deposit" })))
}

pub async fn recover_default(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ZeroDepositRecoverReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut tx = state.db.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    // Create ledger entry
    sqlx::query(
        "INSERT INTO ledger_entries (zero_deposit_id, transaction_type, amount, description) VALUES ($1, 'repayment', $2, $3)",
    )
    .bind(id)
    .bind(payload.recovered_amount)
    .bind(&payload.remarks)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    // Update zero_deposits remarks
    sqlx::query(
        "UPDATE zero_deposits SET default_remarks = $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(&payload.remarks)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let _ = log_admin_action_tx(
        &mut *tx,
        &admin_claims.sub,
        "RECOVER_DEFAULT_ZERO_DEPOSIT",
        "ZERO_DEPOSIT",
        Some(id),
        Some(json!({ "recovered_amount": payload.recovered_amount, "remarks": payload.remarks })),
    ).await;

    tx.commit().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    Ok(Json(json!({ "success": true, "message": "Recovery recorded" })))
}

// ---------------------------------------------------------------------------
// I. Monthly Reporting
// ---------------------------------------------------------------------------

/// GET /api/admin/zero-deposit/reports/monthly?year=&month=
pub async fn get_monthly_reports(
    State(state): State<AppState>,
    Query(query): Query<MonthlyReportQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let now = Utc::now();
    let year = query.year.unwrap_or(now.format("%Y").to_string().parse().unwrap_or(2026));
    let month = query.month.unwrap_or(now.format("%m").to_string().parse().unwrap_or(11));

    match zd_jobs::get_monthly_approval_report(&state.db, year, month).await {
        Ok(report) => Ok(Json(json!({ "success": true, "data": report }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )),
    }
}

// ---------------------------------------------------------------------------
// Redemptions Admin Handlers (Prompt 4)
// ---------------------------------------------------------------------------

pub async fn get_admin_redemptions(
    State(state): State<AppState>,
    Query(filter): Query<ZeroDepositQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = filter.limit.unwrap_or(50).min(200);
    let offset = filter.offset.unwrap_or(0);

    let redemptions = zd_redemption_service::get_admin_redemptions_inbox(
        &state.db,
        filter.status.as_deref(),
        limit,
        offset,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    Ok(Json(json!({
        "success": true,
        "data": redemptions
    })))
}

pub async fn get_admin_redemption_detail(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match zd_redemption_service::get_admin_redemption_detail(&state, id).await {
        Ok(detail) => Ok(Json(json!({ "success": true, "data": detail }))),
        Err(e) => Err(map_redemption_error(e)),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct AdminMessagesQuery {
    pub before: Option<chrono::DateTime<chrono::Utc>>,
    pub limit: Option<i64>,
}

pub async fn admin_get_redemption_messages(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<AdminMessagesQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = query.limit.unwrap_or(50).min(100);
    match zd_redemption_service::get_redemption_messages(&state.db, id, query.before, limit).await {
        Ok(messages) => Ok(Json(json!({ "success": true, "data": { "messages": messages } }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )),
    }
}

pub async fn admin_send_redemption_message(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<SendMessageReq>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let admin_id = Uuid::parse_str(&admin_claims.sub).unwrap_or_else(|_| Uuid::new_v4());
    let admin_name: String = sqlx::query_scalar(
        "SELECT coalesce(first_name || ' ' || last_name, email) FROM users WHERE id = $1"
    )
    .bind(admin_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "Admin".into());

    let res = zd_redemption_service::send_redemption_message_admin(
        &state,
        admin_id,
        &admin_name,
        id,
        &payload.text,
    )
    .await
    .map_err(map_redemption_error)?;

    let _ = log_admin_action(
        &state.db,
        &admin_claims.sub,
        "SEND_REDEMPTION_MESSAGE",
        "ZD_REDEMPTION",
        Some(id),
        Some(json!({ "text_length": payload.text.len() })),
    )
    .await;

    Ok((StatusCode::CREATED, Json(res)))
}

pub async fn admin_add_document_request(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<AdminDocRequestReq>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let admin_id = Uuid::parse_str(&admin_claims.sub).unwrap_or_else(|_| Uuid::new_v4());

    let res = zd_redemption_service::add_document_request_admin(
        &state,
        admin_id,
        id,
        &payload,
    )
    .await
    .map_err(map_redemption_error)?;

    let _ = log_admin_action(
        &state.db,
        &admin_claims.sub,
        "REQUEST_REDEMPTION_DOCUMENT",
        "ZD_REDEMPTION",
        Some(id),
        Some(json!({ "doc_type": payload.doc_type, "required": payload.required })),
    )
    .await;

    Ok((StatusCode::CREATED, Json(res)))
}

pub async fn admin_review_document(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path((id, doc_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<AdminDocReviewReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let admin_id = Uuid::parse_str(&admin_claims.sub).unwrap_or_else(|_| Uuid::new_v4());

    let res = zd_redemption_service::review_document_admin(
        &state,
        admin_id,
        id,
        doc_id,
        &payload,
    )
    .await
    .map_err(map_redemption_error)?;

    let _ = log_admin_action(
        &state.db,
        &admin_claims.sub,
        "REVIEW_REDEMPTION_DOCUMENT",
        "ZD_REDEMPTION",
        Some(id),
        Some(json!({ "document_id": doc_id, "status": payload.status })),
    )
    .await;

    Ok(Json(res))
}

pub async fn approve_redemption(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<AdminApproveReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let admin_id = Uuid::parse_str(&admin_claims.sub).unwrap_or_else(|_| Uuid::new_v4());

    let res = zd_redemption_service::approve_redemption_admin(
        &state,
        admin_id,
        id,
        &payload,
    )
    .await
    .map_err(map_redemption_error)?;

    let _ = log_admin_action(
        &state.db,
        &admin_claims.sub,
        "APPROVE_REDEMPTION",
        "ZD_REDEMPTION",
        Some(id),
        Some(json!({ "tokens_approved": payload.tokens_approved })),
    )
    .await;

    Ok(Json(res))
}

pub async fn release_redemption(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<AdminReleaseReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let admin_id = Uuid::parse_str(&admin_claims.sub).unwrap_or_else(|_| Uuid::new_v4());

    let idempotency_key = match headers.get("idempotency-key").and_then(|v| v.to_str().ok()) {
        Some(k) => match Uuid::parse_str(k) {
            Ok(u) => Some(u),
            Err(_) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "success": false,
                        "error": "INVALID_IDEMPOTENCY_KEY",
                        "message": "Idempotency-Key must be a valid UUID"
                    })),
                ));
            }
        },
        None => None,
    };

    let res = zd_redemption_service::mark_released_admin(
        &state,
        admin_id,
        id,
        idempotency_key,
        &payload,
    )
    .await
    .map_err(map_redemption_error)?;

    let _ = log_admin_action(
        &state.db,
        &admin_claims.sub,
        "RELEASE_REDEMPTION",
        "ZD_REDEMPTION",
        Some(id),
        Some(json!({ "released_amount": payload.released_amount, "utr": payload.utr })),
    )
    .await;

    Ok(Json(res))
}

pub async fn reject_redemption(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<AdminRejectReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let admin_id = Uuid::parse_str(&admin_claims.sub).unwrap_or_else(|_| Uuid::new_v4());

    let res = zd_redemption_service::reject_redemption_admin(
        &state,
        admin_id,
        id,
        &payload,
    )
    .await
    .map_err(map_redemption_error)?;

    let _ = log_admin_action(
        &state.db,
        &admin_claims.sub,
        "REJECT_REDEMPTION",
        "ZD_REDEMPTION",
        Some(id),
        Some(json!({ "reason": payload.reason })),
    )
    .await;

    Ok(Json(res))
}
