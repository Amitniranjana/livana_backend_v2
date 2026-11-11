use axum::{
    extract::{Path, Query, State, Extension},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::admin_zero_deposit::{
        ZeroDepositApproveReq,
        ZeroDepositDefaultActionReq, ZeroDepositQuery, ZeroDepositRecoverReq, ZeroDepositRejectReq,
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
