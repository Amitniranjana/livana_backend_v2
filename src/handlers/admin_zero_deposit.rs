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
        NbfcPartnerReq, NbfcPartnerUpdateReq, ZeroDepositApproveReq, ZeroDepositAssignNbfcReq,
        ZeroDepositDefaultActionReq, ZeroDepositQuery, ZeroDepositRecoverReq, ZeroDepositRejectReq,
    },
    handlers::admin_auth::AdminClaims,
    models::nbfc::NbfcPartner,
    utils::admin_logger::log_admin_action,
};

pub async fn get_zero_deposits(
    State(state): State<AppState>,
    Query(filter): Query<ZeroDepositQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut inner_query = String::from("SELECT * FROM zero_deposits WHERE 1=1");
    
    if let Some(status) = &filter.status {
        inner_query.push_str(&format!(" AND status = '{}'", status));
    }

    if let Some(limit) = filter.limit {
        inner_query.push_str(&format!(" LIMIT {}", limit));
    }
    
    if let Some(offset) = filter.offset {
        inner_query.push_str(&format!(" OFFSET {}", offset));
    }

    let query = format!("SELECT row_to_json(t) FROM ({}) t", inner_query);

    let records = sqlx::query_scalar::<_, serde_json::Value>(&query)
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
                   k.document_urls as kyc_document_urls
            FROM zero_deposits z
            JOIN users u ON z.user_id = u.id
            LEFT JOIN properties p ON z.property_id = p.id
            JOIN kyc_submissions k ON z.kyc_id = k.id
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

    let res = sqlx::query(
        "UPDATE zero_deposits SET status = 'approved', approved_deposit_amount = $1, admin_remarks = $2, updated_at = NOW() WHERE id = $3",
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
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "Not found" })),
        ));
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

    Ok(Json(json!({ "success": true, "message": "Zero Deposit approved" })))
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

    let res = sqlx::query(
        "UPDATE zero_deposits SET status = 'rejected', admin_remarks = $1, updated_at = NOW() WHERE id = $2",
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
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "Not found" })),
        ));
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

pub async fn assign_nbfc(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ZeroDepositAssignNbfcReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let res = sqlx::query(
        "UPDATE zero_deposits SET nbfc_id = $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(payload.nbfc_id)
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
        "ASSIGN_NBFC_ZERO_DEPOSIT",
        "ZERO_DEPOSIT",
        Some(id),
        Some(json!({ "nbfc_id": payload.nbfc_id })),
    ).await;

    Ok(Json(json!({ "success": true, "message": "Assigned to NBFC" })))
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
    let mut inner_query = String::from("SELECT * FROM zero_deposits WHERE is_defaulted = true");
    
    if let Some(limit) = filter.limit {
        inner_query.push_str(&format!(" LIMIT {}", limit));
    }
    
    if let Some(offset) = filter.offset {
        inner_query.push_str(&format!(" OFFSET {}", offset));
    }

    let query = format!("SELECT row_to_json(t) FROM ({}) t", inner_query);

    let records = sqlx::query_scalar::<_, serde_json::Value>(&query)
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

// NBFC Partners

pub async fn list_nbfc_partners(
    State(state): State<AppState>,
) -> Result<Json<Vec<NbfcPartner>>, (StatusCode, Json<Value>)> {
    let records = sqlx::query_as::<_, NbfcPartner>("SELECT * FROM nbfc_partners ORDER BY created_at DESC")
        .fetch_all(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": e.to_string() })),
            )
        })?;

    Ok(Json(records))
}

pub async fn onboard_nbfc_partner(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Json(payload): Json<NbfcPartnerReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let record = sqlx::query_as::<_, NbfcPartner>(
        r#"
        INSERT INTO nbfc_partners (name, entity_type, revenue_share_pct, fldg_buffer_pct)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#
    )
    .bind(payload.name)
    .bind(payload.entity_type)
    .bind(payload.revenue_share_pct)
    .bind(payload.fldg_buffer_pct)
    .fetch_one(&state.db)
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
        "ONBOARD_NBFC_PARTNER",
        "NBFC_PARTNER",
        Some(record.id),
        Some(json!({ "name": record.name })),
    ).await;

    Ok(Json(json!({ "success": true, "data": record })))
}

pub async fn update_nbfc_partner(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<NbfcPartnerUpdateReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let res = sqlx::query(
        "UPDATE nbfc_partners SET status = $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(payload.status.clone() as crate::models::nbfc::NbfcStatus)
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
        "UPDATE_NBFC_PARTNER_STATUS",
        "NBFC_PARTNER",
        Some(id),
        Some(json!({ "status": payload.status })),
    ).await;

    Ok(Json(json!({ "success": true, "message": "NBFC Partner updated" })))
}
