use axum::{
    extract::{Path, Query, State, Extension},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::admin_zero_deposit::{
        RedemptionApproveReq, RedemptionReleaseReq, RedemptionRejectReq,
        ZeroDepositApproveReq, ZeroDepositDefaultActionReq, ZeroDepositQuery,
        ZeroDepositRecoverReq, ZeroDepositRejectReq,
    },
    handlers::admin_auth::AdminClaims,
    utils::admin_logger::log_admin_action,
};

pub async fn get_zero_deposits(
    State(state): State<AppState>,
    Query(filter): Query<ZeroDepositQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Build query with bound parameters (Issue 106 — no format! injection)
    let status_filter = filter.status.as_deref();
    let limit = filter.limit.unwrap_or(50).min(200);
    let offset = filter.offset.unwrap_or(0);

    let records = if let Some(status) = status_filter {
        sqlx::query_scalar::<_, serde_json::Value>(
            r#"SELECT row_to_json(t) FROM (
                SELECT * FROM zero_deposits WHERE status::TEXT = $1
                ORDER BY created_at DESC LIMIT $2 OFFSET $3
            ) t"#,
        )
        .bind(status)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_scalar::<_, serde_json::Value>(
            r#"SELECT row_to_json(t) FROM (
                SELECT * FROM zero_deposits
                ORDER BY created_at DESC LIMIT $1 OFFSET $2
            ) t"#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
    }
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

pub async fn get_zero_deposit_detail(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let record = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT row_to_json(t) FROM (
            SELECT z.*, 
                   u.first_name as user_first_name, u.last_name as user_last_name, u.email as user_email, u.phone_no as user_phone,
                   p.title as property_title,
                   (
                       SELECT coalesce(array_agg(doc), ARRAY[]::text[])
                       FROM unnest(ARRAY[k.profile_picture_url, k.govt_id_document_url, k.experience_document_url]) as doc
                       WHERE doc IS NOT NULL
                   ) as kyc_document_urls
            FROM zero_deposits z
            JOIN users u ON z.user_id = u.id
            LEFT JOIN properties p ON z.property_id = p.id
            LEFT JOIN kyc_submissions k ON z.kyc_id = k.id
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
            Json(json!({ "success": false, "error": "Zero Deposit application not found" })),
        ))
    }
}

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

    // Issue 108: SQL-enforced transition WHERE status IN (...)
    let res = sqlx::query(
        "UPDATE zero_deposits SET status = 'approved'::zero_deposit_status, approved_deposit_amount = $1, admin_remarks = $2, updated_at = NOW() WHERE id = $3 AND status IN ('ai_review'::zero_deposit_status, 'ai_manual_review'::zero_deposit_status, 'pending_review'::zero_deposit_status)",
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

    if res.rows_affected() == 0 {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM zero_deposits WHERE id = $1)")
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(false);

        if exists {
            return Err((
                StatusCode::CONFLICT,
                Json(json!({ "success": false, "error": "INVALID_STATE", "message": "Application cannot be approved from its current status" })),
            ));
        } else {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "error": "NOT_FOUND", "message": "Zero Deposit application not found" })),
            ));
        }
    }

    let admin_id = admin_claims.sub.to_string();
    let _ = log_admin_action(
        &state.db,
        &admin_id,
        "APPROVE_ZERO_DEPOSIT",
        "ZERO_DEPOSIT",
        Some(id),
        Some(json!({ "approved_deposit_amount": payload.approved_deposit_amount, "remarks": payload.remarks })),
    ).await;

    tx.commit().await.unwrap();

    // Issue 109: Issue tokens and advance status to tokens_issued
    let admin_uuid = Uuid::parse_str(&admin_id).ok();
    let monthly_rent = payload.approved_deposit_amount / crate::services::zd_token_service::DEPOSIT_RENT_MULTIPLE;
    let _ = crate::services::zd_token_service::issue(
        &state.db,
        id,
        monthly_rent,
        Some("admin_override"),
        admin_uuid,
        None,
        Some("Admin approved"),
    ).await;

    let _ = sqlx::query(
        "UPDATE zero_deposits SET status = 'tokens_issued'::zero_deposit_status, updated_at = NOW() WHERE id = $1 AND status = 'approved'::zero_deposit_status"
    )
    .bind(id)
    .execute(&state.db)
    .await;

    Ok(Json(json!({ "success": true, "message": "Zero Deposit approved and tokens issued" })))
}

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

    // Issue 108: SQL-enforced transition WHERE status NOT IN ('rejected', 'released', 'expired')
    let res = sqlx::query(
        "UPDATE zero_deposits SET status = 'rejected'::zero_deposit_status, admin_remarks = $1, updated_at = NOW() WHERE id = $2 AND status NOT IN ('rejected'::zero_deposit_status, 'released'::zero_deposit_status, 'expired'::zero_deposit_status)",
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

    let admin_id = admin_claims.sub.to_string();
    let _ = log_admin_action(
        &state.db,
        &admin_id,
        "REJECT_ZERO_DEPOSIT",
        "ZERO_DEPOSIT",
        Some(id),
        Some(json!({ "reason": payload.reason })),
    ).await;

    tx.commit().await.unwrap();

    Ok(Json(json!({ "success": true, "message": "Zero Deposit rejected" })))
}

pub async fn get_zero_deposit_ledger(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let records = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT row_to_json(t) FROM (SELECT id, transaction_type, amount, reference_id, description, created_at FROM ledger_entries WHERE zero_deposit_id = $1 ORDER BY created_at DESC) t",
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
    let res = sqlx::query(
        "UPDATE zero_deposits SET is_defaulted = true, default_remarks = $1, status = 'blocked', updated_at = NOW() WHERE id = $2",
    )
    .bind(&payload.remarks)
    .bind(id)
    .execute(&state.db)
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

    let admin_id = admin_claims.sub.to_string();
    let _ = log_admin_action(
        &state.db,
        &admin_id,
        "SUSPEND_DEFAULT_ZERO_DEPOSIT",
        "ZERO_DEPOSIT",
        Some(id),
        Some(json!({ "remarks": payload.remarks })),
    ).await;

    Ok(Json(json!({ "success": true, "message": "Suspended defaulted Zero Deposit" })))
}

pub async fn forfeit_default(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ZeroDepositDefaultActionReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let res = sqlx::query(
        "UPDATE zero_deposits SET default_remarks = $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(&payload.remarks)
    .bind(id)
    .execute(&state.db)
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

    let admin_id = admin_claims.sub.to_string();
    let _ = log_admin_action(
        &state.db,
        &admin_id,
        "FORFEIT_DEFAULT_ZERO_DEPOSIT",
        "ZERO_DEPOSIT",
        Some(id),
        Some(json!({ "remarks": payload.remarks })),
    ).await;

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

    let admin_id = admin_claims.sub.to_string();
    let _ = log_admin_action(
        &state.db,
        &admin_id,
        "RECOVER_DEFAULT_ZERO_DEPOSIT",
        "ZERO_DEPOSIT",
        Some(id),
        Some(json!({ "recovered_amount": payload.recovered_amount, "remarks": payload.remarks })),
    ).await;

    tx.commit().await.unwrap();

    Ok(Json(json!({ "success": true, "message": "Recovery recorded" })))
}

pub async fn get_admin_redemptions(
    State(state): State<AppState>,
    Query(filter): Query<ZeroDepositQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let status_filter = filter.status.as_deref();
    let limit = filter.limit.unwrap_or(50).min(200);
    let offset = filter.offset.unwrap_or(0);

    let records = if let Some(status) = status_filter {
        sqlx::query_scalar::<_, serde_json::Value>(
            r#"SELECT row_to_json(t) FROM (
                SELECT r.*,
                       u.first_name as user_first_name, u.last_name as user_last_name, u.email as user_email, u.phone_no as user_phone,
                       z.monthly_rent as application_monthly_rent, z.deposit_amount as application_deposit_amount,
                       (
                           SELECT coalesce(json_agg(row_to_json(d)), '[]'::json)
                           FROM zd_redemption_documents d
                           WHERE d.redemption_id = r.id
                       ) as documents
                FROM zd_redemptions r
                JOIN zero_deposits z ON r.zero_deposit_id = z.id
                LEFT JOIN users u ON coalesce(r.user_id, z.user_id) = u.id
                WHERE r.status::TEXT = $1
                ORDER BY r.created_at DESC
                LIMIT $2 OFFSET $3
            ) t"#,
        )
        .bind(status)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_scalar::<_, serde_json::Value>(
            r#"SELECT row_to_json(t) FROM (
                SELECT r.*,
                       u.first_name as user_first_name, u.last_name as user_last_name, u.email as user_email, u.phone_no as user_phone,
                       z.monthly_rent as application_monthly_rent, z.deposit_amount as application_deposit_amount,
                       (
                           SELECT coalesce(json_agg(row_to_json(d)), '[]'::json)
                           FROM zd_redemption_documents d
                           WHERE d.redemption_id = r.id
                       ) as documents
                FROM zd_redemptions r
                JOIN zero_deposits z ON r.zero_deposit_id = z.id
                LEFT JOIN users u ON coalesce(r.user_id, z.user_id) = u.id
                ORDER BY r.created_at DESC
                LIMIT $1 OFFSET $2
            ) t"#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
    }
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

pub async fn get_admin_redemption_detail(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let record = sqlx::query_scalar::<_, serde_json::Value>(
        r#"SELECT row_to_json(t) FROM (
            SELECT r.*,
                   u.first_name as user_first_name, u.last_name as user_last_name, u.email as user_email, u.phone_no as user_phone,
                   z.monthly_rent as application_monthly_rent, z.deposit_amount as application_deposit_amount,
                   (
                       SELECT coalesce(json_agg(row_to_json(d)), '[]'::json)
                       FROM zd_redemption_documents d
                       WHERE d.redemption_id = r.id
                   ) as documents,
                   (
                       SELECT row_to_json(a)
                       FROM zd_token_allocations a
                       WHERE a.id = r.allocation_id
                   ) as token_allocation
            FROM zd_redemptions r
            JOIN zero_deposits z ON r.zero_deposit_id = z.id
            LEFT JOIN users u ON coalesce(r.user_id, z.user_id) = u.id
            WHERE r.id = $1
        ) t"#,
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
            Json(json!({
                "success": false,
                "error": "NOT_FOUND",
                "message": "Redemption not found"
            })),
        ))
    }
}

pub async fn approve_redemption(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<RedemptionApproveReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut tx = state.db.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let redemption = sqlx::query(
        "SELECT status::TEXT as status, zero_deposit_id, allocation_id, tokens_requested::FLOAT8 as tokens_requested
         FROM zd_redemptions WHERE id = $1 FOR UPDATE",
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

    let redemption = match redemption {
        Some(r) => r,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": "NOT_FOUND",
                    "message": "Redemption not found"
                })),
            ));
        }
    };

    let current_status: String = redemption.get("status");
    let tokens_requested: f64 = redemption.get("tokens_requested");

    if current_status != "under_review" && current_status != "docs_pending" && current_status != "submitted" && current_status != "admin_review" {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "INVALID_STATE",
                "message": format!("Redemption cannot be approved from status '{}'", current_status)
            })),
        ));
    }

    if payload.tokens_approved > tokens_requested {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "INSUFFICIENT_TOKENS",
                "message": "Tokens approved cannot exceed tokens requested"
            })),
        ));
    }

    let payee_snapshot = json!({
        "payee_name": payload.payee_name,
        "payee_account_number": payload.payee_account_number,
        "payee_ifsc": payload.payee_ifsc,
    });

    let admin_uuid = Uuid::parse_str(&admin_claims.sub).ok();

    sqlx::query(
        r#"UPDATE zd_redemptions
           SET status = 'approved'::zd_redemption_status,
               tokens_approved = $1,
               payee_snapshot = $2,
               approved_by = $3,
               approved_at = NOW(),
               admin_remarks = coalesce($4, admin_remarks),
               updated_at = NOW()
           WHERE id = $5"#
    )
    .bind(payload.tokens_approved)
    .bind(payee_snapshot)
    .bind(admin_uuid)
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

    let _ = log_admin_action(
        &state.db,
        &admin_claims.sub,
        "APPROVE_REDEMPTION",
        "ZD_REDEMPTION",
        Some(id),
        Some(json!({
            "tokens_approved": payload.tokens_approved,
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

    Ok(Json(json!({
        "success": true,
        "message": "Redemption approved successfully",
        "data": {
            "redemption_id": id,
            "status": "approved",
            "tokens_approved": payload.tokens_approved
        }
    })))
}

pub async fn release_redemption(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<RedemptionReleaseReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut tx = state.db.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let redemption = sqlx::query(
        "SELECT status::TEXT as status, zero_deposit_id, allocation_id, approved_by,
                tokens_requested::FLOAT8 as tokens_requested,
                coalesce(tokens_approved::FLOAT8, tokens_requested::FLOAT8) as tokens_approved
         FROM zd_redemptions WHERE id = $1 FOR UPDATE",
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

    let redemption = match redemption {
        Some(r) => r,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": "NOT_FOUND",
                    "message": "Redemption not found"
                })),
            ));
        }
    };

    let current_status: String = redemption.get("status");
    let zero_deposit_id: Uuid = redemption.get("zero_deposit_id");
    let approved_by: Option<Uuid> = redemption.get("approved_by");
    let tokens_requested: f64 = redemption.get("tokens_requested");
    let tokens_approved: f64 = redemption.get("tokens_approved");

    if current_status != "approved" {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "INVALID_STATE",
                "message": format!("Redemption must be approved before release (current status is '{}')", current_status)
            })),
        ));
    }

    let admin_uuid = Uuid::parse_str(&admin_claims.sub).ok();

    // Maker-checker enforcement
    if let (Some(approver), Some(checker)) = (approved_by, admin_uuid) {
        if approver == checker {
            return Err((
                StatusCode::FORBIDDEN,
                Json(json!({
                    "success": false,
                    "error": "MAKER_CHECKER",
                    "message": "Maker-checker violation: A different admin must release funds"
                })),
            ));
        }
    }

    let utr_clean = payload.utr.trim();
    if utr_clean.is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "success": false,
                "error": "PRECONDITION_FAILED",
                "message": "Bank UTR cannot be empty"
            })),
        ));
    }

    let utr_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM zd_redemptions WHERE utr = $1 AND id != $2)"
    )
    .bind(utr_clean)
    .bind(id)
    .fetch_one(&mut *tx)
    .await
    .unwrap_or(false);

    if utr_exists {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "DUPLICATE_REQUEST",
                "message": format!("Bank UTR '{}' has already been recorded for another redemption", utr_clean)
            })),
        ));
    }

    sqlx::query(
        r#"UPDATE zd_redemptions
           SET status = 'released'::zd_redemption_status,
               released_by = $1,
               released_at = NOW(),
               released_amount = $2,
               payout_mode = $3,
               utr = $4,
               completed_at = NOW(),
               admin_remarks = coalesce($5, admin_remarks),
               updated_at = NOW()
           WHERE id = $6 AND status = 'approved'::zd_redemption_status"#
    )
    .bind(admin_uuid)
    .bind(payload.released_amount)
    .bind(payload.payout_mode.as_deref().unwrap_or("NEFT"))
    .bind(utr_clean)
    .bind(&payload.notes)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    sqlx::query(
        "UPDATE zero_deposits
         SET status = 'released'::zero_deposit_status, updated_at = NOW()
         WHERE id = $1"
    )
    .bind(zero_deposit_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let _ = sqlx::query(
        r#"INSERT INTO ledger_entries (id, zero_deposit_id, transaction_type, amount, reference_id, description, created_at)
           VALUES (gen_random_uuid(), $1, 'disbursement', $2, $3, $4, NOW())"#
    )
    .bind(zero_deposit_id)
    .bind(payload.released_amount)
    .bind(utr_clean)
    .bind(format!("Redemption payout to landlord via {}", payload.payout_mode.as_deref().unwrap_or("NEFT")))
    .execute(&mut *tx)
    .await;

    let _ = log_admin_action(
        &state.db,
        &admin_claims.sub,
        "RELEASE_REDEMPTION",
        "ZD_REDEMPTION",
        Some(id),
        Some(json!({
            "released_amount": payload.released_amount,
            "utr": utr_clean,
            "payout_mode": payload.payout_mode,
            "notes": payload.notes
        })),
    )
    .await;

    tx.commit().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let _ = crate::services::zd_token_service::redeem(
        &state.db,
        zero_deposit_id,
        tokens_requested,
        tokens_approved,
        id,
        admin_uuid,
        Some("admin"),
        payload.notes.as_deref(),
    )
    .await;

    Ok(Json(json!({
        "success": true,
        "message": "Redemption funds released and UTR recorded",
        "data": {
            "redemption_id": id,
            "status": "released",
            "utr": utr_clean,
            "released_amount": payload.released_amount
        }
    })))
}

pub async fn reject_redemption(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<RedemptionRejectReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut tx = state.db.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
    })?;

    let redemption = sqlx::query(
        "SELECT status::TEXT as status, zero_deposit_id, tokens_requested::FLOAT8 as tokens_requested
         FROM zd_redemptions WHERE id = $1 FOR UPDATE",
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

    let redemption = match redemption {
        Some(r) => r,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": "NOT_FOUND",
                    "message": "Redemption not found"
                })),
            ));
        }
    };

    let current_status: String = redemption.get("status");
    let zero_deposit_id: Uuid = redemption.get("zero_deposit_id");
    let tokens_requested: f64 = redemption.get("tokens_requested");

    if current_status == "released" || current_status == "rejected" || current_status == "cancelled" {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "INVALID_STATE",
                "message": format!("Redemption cannot be rejected from status '{}'", current_status)
            })),
        ));
    }

    let admin_uuid = Uuid::parse_str(&admin_claims.sub).ok();

    sqlx::query(
        r#"UPDATE zd_redemptions
           SET status = 'rejected'::zd_redemption_status,
               reject_reason = $1,
               completed_at = NOW(),
               updated_at = NOW()
           WHERE id = $2"#
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

    let _ = log_admin_action(
        &state.db,
        &admin_claims.sub,
        "REJECT_REDEMPTION",
        "ZD_REDEMPTION",
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

    let _ = crate::services::zd_token_service::unlock(
        &state.db,
        zero_deposit_id,
        tokens_requested,
        id,
        admin_uuid,
        Some("admin"),
        Some(&payload.reason),
    )
    .await;

    Ok(Json(json!({
        "success": true,
        "message": "Redemption rejected and tokens returned/expired",
        "data": {
            "redemption_id": id,
            "status": "rejected"
        }
    })))
}
