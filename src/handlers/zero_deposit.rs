use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Extension, Json,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    models::zero_deposit::{ApplyZeroDepositRequest, CreditCheckConsentRequest},
    services::zero_deposit_service,
    utils::loan_velocity::LoanApplyContext,
};

#[derive(serde::Deserialize)]
struct Claims {
    sub: String,
    #[allow(dead_code)]
    exp: usize,
}

fn extract_user_id_from_jwt(token: &str, key: &DecodingKey) -> Result<Uuid, String> {
    let data = decode::<Claims>(token, key, &Validation::default()).map_err(|e| e.to_string())?;
    Uuid::parse_str(&data.claims.sub).map_err(|e| e.to_string())
}

pub fn require_auth(
    headers: &HeaderMap,
    jwt_secret: &str,
) -> Result<Uuid, (StatusCode, axum::Json<serde_json::Value>)> {
    let bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").map(|t| t.to_string()));

    let token = bearer.ok_or_else(|| {
        let body = json!({"success": false, "message": "Missing or invalid Authorization header", "error_code": "UNAUTHORIZED"});
        (StatusCode::UNAUTHORIZED, Json(body))
    })?;

    extract_user_id_from_jwt(&token, &DecodingKey::from_secret(jwt_secret.as_bytes()))
        .map_err(|e| {
            let body = json!({"success": false, "message": format!("Auth error: {}", e), "error_code": "INVALID_TOKEN"});
            (StatusCode::UNAUTHORIZED, Json(body))
        })
}

/// POST /api/v1/zero_deposits/apply
///
/// The velocity-check middleware (`loan_velocity_guard`) runs *before* this
/// handler and injects a `LoanApplyContext` extension containing the
/// device_id and client_ip already validated against rate limits (Issue 97).
pub async fn apply_zero_deposit(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    // Issue 97: injected by loan_velocity_guard middleware
    apply_ctx: Option<Extension<LoanApplyContext>>,
    Json(payload): Json<ApplyZeroDepositRequest>,
) -> impl axum::response::IntoResponse {
    let user_id = match require_auth(&headers, &app_state.jwt_secret) {
        Ok(uid) => uid,
        Err((code, body)) => return (code, body),
    };

    // Extract velocity context (device_id + client_ip) injected by middleware.
    // Falls back to None gracefully when the middleware is not applied
    // (e.g. in tests or older route configurations).
    let (ctx_device_id, client_ip) = apply_ctx
        .map(|Extension(ctx)| (ctx.device_id.clone(), ctx.client_ip.clone()))
        .unwrap_or((None, None));
    let device_id = ctx_device_id.or_else(|| payload.device_fingerprint.clone());

    // Validation 1: Tenancy duration
    if payload.tenancy_months < 11 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "success": false,
                "error": "TENANCY_TOO_SHORT",
                "message": "Tenancy duration must be at least 11 months"
            })),
        );
    }

    // Validation 2: Employment documents
    let docs = payload.documents.as_ref();
    if payload.employment_type == "salaried" {
        let has_slips = docs.map(|d| d.salary_slip_urls.len() >= 3).unwrap_or(false);
        let has_offer = docs.and_then(|d| d.offer_letter_url.as_ref()).is_some();
        let has_bank = docs.and_then(|d| d.bank_statement_url.as_ref()).is_some() || payload.bank_statement_url.is_some();

        if !(has_slips || has_offer) || !has_bank {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "success": false,
                    "error": "DOCUMENTS_MISSING",
                    "message": "Salaried applicants require 3 salary slips or offer letter, plus 6 months bank statement"
                })),
            );
        }
    } else if payload.employment_type == "self_employed" {
        let has_gst = docs.map(|d| !d.gst_return_urls.is_empty()).unwrap_or(false);
        let has_itr = docs.and_then(|d| d.itr_url.as_ref()).is_some() || payload.itr_document_url.is_some();
        let has_bank = docs.and_then(|d| d.bank_statement_url.as_ref()).is_some() || payload.bank_statement_url.is_some();

        if !has_gst && !has_itr && !has_bank {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "success": false,
                    "error": "DOCUMENTS_MISSING",
                    "message": "Self-employed applicants require GST returns, ITR, or 12 months bank statements"
                })),
            );
        }
    }

    let kyc_id = match Uuid::parse_str(&payload.kyc_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "Invalid kyc_id", "error_code": "INVALID_UUID"})),
            );
        }
    };

    let property_id = match &payload.property_id {
        Some(pid_str) => match Uuid::parse_str(pid_str) {
            Ok(u) => Some(u),
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"success": false, "message": "Invalid property_id", "error_code": "INVALID_UUID"})),
                );
            }
        },
        None => None,
    };

    let deposit_amount = payload.monthly_rent * 3.0;
    let fee_amount = payload.monthly_rent + 2000.0;
    let first_rent_amount = payload.monthly_rent;

    match zero_deposit_service::apply_for_zero_deposit(
        &app_state.db,
        user_id,
        property_id,
        kyc_id,
        &payload,
        deposit_amount,
        fee_amount,
        device_id.as_deref(),
        client_ip.as_deref(),
    )
    .await
    {
        Ok(zero_deposit) => (
            StatusCode::CREATED,
            Json(json!({
                "success": true,
                "message": "zero_deposit application submitted successfully",
                "data": {
                    "zero_deposit": zero_deposit,
                    "deposit_amount": deposit_amount,
                    "fee_amount": fee_amount,
                    "first_rent_amount": first_rent_amount
                }
            })),
        ),
        Err(e) => {
            log::error!("apply_zero_deposit DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Failed to submit zero_deposit application",
                    "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

/// POST /api/v1/zero_deposits/credit-check/consent
pub async fn credit_check_consent(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreditCheckConsentRequest>,
) -> impl axum::response::IntoResponse {
    let user_id = match require_auth(&headers, &app_state.jwt_secret) {
        Ok(uid) => uid,
        Err((code, body)) => return (code, body),
    };

    let zero_deposit_id = match Uuid::parse_str(&payload.zero_deposit_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "Invalid zero_deposit_id", "error_code": "INVALID_UUID"})),
            );
        }
    };

    match zero_deposit_service::update_consent(&app_state.db, zero_deposit_id, user_id, payload.consent_given).await {
        Ok(zero_deposit_service::ConsentUpdateResult::Success(zero_deposit)) => {
            if payload.consent_given {
                let db_clone = app_state.db.clone();
                let redis_clone = app_state.redis_pool.clone();
                tokio::spawn(async move {
                    let _ = crate::services::zd_ai_client_service::queue_ai_assessment_job(
                        &db_clone,
                        redis_clone,
                        zero_deposit_id,
                        user_id,
                    )
                    .await;
                });

                (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "message": "zero_deposit consent updated successfully",
                        "data": {
                            "zero_deposit": zero_deposit,
                            "assessment_status": "queued"
                        }
                    })),
                )
            } else {
                (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "message": "zero_deposit consent updated successfully",
                        "data": {
                            "zero_deposit": zero_deposit,
                            "assessment_status": "blocked"
                        }
                    })),
                )
            }
        }
        Ok(zero_deposit_service::ConsentUpdateResult::NotOwner) => (
            StatusCode::FORBIDDEN,
            Json(json!({
                "success": false,
                "error": "NOT_OWNER",
                "message": "You do not own this zero_deposit application"
            })),
        ),
        Ok(zero_deposit_service::ConsentUpdateResult::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": "NOT_FOUND",
                "message": "zero_deposit application not found"
            })),
        ),
        Err(e) => {
            log::error!("credit_check_consent DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Database error",
                    "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

/// GET /api/v1/zero_deposits/{zero_deposit_id}
pub async fn get_zero_deposit(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(zero_deposit_id): Path<String>,
) -> impl axum::response::IntoResponse {
    let user_id = match require_auth(&headers, &app_state.jwt_secret) {
        Ok(uid) => uid,
        Err((code, body)) => return (code, body),
    };

    let zero_deposit_id = match Uuid::parse_str(&zero_deposit_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "Invalid zero_deposit_id", "error_code": "INVALID_UUID"})),
            );
        }
    };

    match zero_deposit_service::get_zero_deposit_by_id(&app_state.db, zero_deposit_id, user_id).await {
        Ok(Some(zero_deposit)) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "zero_deposit retrieved successfully",
                "data": { "zero_deposit": zero_deposit }
            })),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "message": "zero_deposit not found or unauthorized",
                "error_code": "NOT_FOUND"
            })),
        ),
        Err(e) => {
            log::error!("get_zero_deposit DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Database error",
                    "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

/// GET /api/v1/zero-deposit/me
pub async fn get_my_zero_deposits(
    State(app_state): State<AppState>,
    headers: HeaderMap,
) -> impl axum::response::IntoResponse {
    let user_id = match require_auth(&headers, &app_state.jwt_secret) {
        Ok(uid) => uid,
        Err((code, body)) => return (code, body),
    };

    match crate::services::zero_deposit_service::get_my_zero_deposits(&app_state.db, user_id).await {
        Ok(zero_deposits) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Zero deposits retrieved successfully",
                "data": { "zero_deposits": zero_deposits }
            })),
        ),
        Err(e) => {
            log::error!("get_my_zero_deposits DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Database error",
                    "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

/// GET /api/v1/zero-deposit/{application_id}/status
pub async fn get_status(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<String>,
) -> impl axum::response::IntoResponse {
    let user_id = match require_auth(&headers, &app_state.jwt_secret) {
        Ok(uid) => uid,
        Err((code, body)) => return (code, body),
    };

    let zero_deposit_id = match Uuid::parse_str(&application_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "Invalid zero_deposit_id", "error_code": "INVALID_UUID"})),
            );
        }
    };

    match crate::services::zero_deposit_service::get_zero_deposit_status(&app_state.db, zero_deposit_id, user_id).await {
        Ok(Some(status_response)) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Status retrieved successfully",
                "data": { "zero_deposit": status_response }
            })),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "message": "Zero deposit not found or unauthorized",
                "error_code": "NOT_FOUND"
            })),
        ),
        Err(e) => {
            log::error!("get_status DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Database error",
                    "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

/// POST /api/v1/zero-deposit/{application_id}/fee/charge
pub async fn charge_fee(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<String>,
    Json(payload): Json<crate::models::zero_deposit::ChargeFeeRequest>,
) -> impl axum::response::IntoResponse {
    let user_id = match require_auth(&headers, &app_state.jwt_secret) {
        Ok(uid) => uid,
        Err((code, body)) => return (code, body),
    };

    let zero_deposit_id = match Uuid::parse_str(&application_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "Invalid zero_deposit_id", "error_code": "INVALID_UUID"})),
            );
        }
    };

    // Check application exists and verify ownership
    let app_row = match sqlx::query(
        "SELECT user_id, monthly_rent::FLOAT8 as rent FROM zero_deposits WHERE id = $1"
    )
    .bind(zero_deposit_id)
    .fetch_optional(&app_state.db)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"success": false, "message": "Zero deposit not found", "error_code": "NOT_FOUND"})),
            );
        }
        Err(e) => {
            log::error!("charge_fee query error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Database error", "error_code": "DB_ERROR"})),
            );
        }
    };

    let owner_id: Uuid = app_row.get("user_id");
    if owner_id != user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"success": false, "message": "Not authorized to charge fee for this application", "error_code": "NOT_OWNER"})),
        );
    }

    let monthly_rent: f64 = app_row.try_get("rent").unwrap_or(0.0);
    // Spec: fee_amount = rent + ₹2,000; first_rent_amount = rent; total = fee_amount + first_rent_amount
    let fee_amount = monthly_rent + 2000.0;
    let first_rent_amount = monthly_rent;
    let total_amount = fee_amount + first_rent_amount;

    match crate::services::zero_deposit_service::initiate_fee_charge(
        &app_state.db,
        zero_deposit_id,
        user_id,
        &payload.upi_vpa,
        total_amount,
    )
    .await
    {
        Ok(Some(fee_txn)) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Fee and first rent charge initiated successfully",
                "data": {
                    "fee_transaction_id": fee_txn.id,
                    "fee_amount": fee_amount,
                    "first_rent_amount": first_rent_amount,
                    "total_amount": total_amount,
                    "status": fee_txn.status,
                    "upi_vpa": fee_txn.upi_vpa,
                    "created_at": fee_txn.created_at
                }
            })),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "message": "Zero deposit not found or unauthorized",
                "error_code": "NOT_FOUND"
            })),
        ),
        Err(e) => {
            log::error!("charge_fee DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Failed to initiate fee charge",
                    "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

/// POST /api/v1/zero-deposit/{application_id}/autopay/setup
pub async fn setup_autopay(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<String>,
    Json(payload): Json<crate::models::zero_deposit::SetupAutopayRequest>,
) -> impl axum::response::IntoResponse {
    let user_id = match require_auth(&headers, &app_state.jwt_secret) {
        Ok(uid) => uid,
        Err((code, body)) => return (code, body),
    };

    let zero_deposit_id = match Uuid::parse_str(&application_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "Invalid zero_deposit_id", "error_code": "INVALID_UUID"})),
            );
        }
    };

    // Check application exists and verify ownership
    let owner_id = match sqlx::query_scalar::<_, Uuid>(
        "SELECT user_id FROM zero_deposits WHERE id = $1"
    )
    .bind(zero_deposit_id)
    .fetch_optional(&app_state.db)
    .await
    {
        Ok(Some(uid)) => uid,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"success": false, "message": "Zero deposit not found", "error_code": "NOT_FOUND"})),
            );
        }
        Err(e) => {
            log::error!("setup_autopay query error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Database error", "error_code": "DB_ERROR"})),
            );
        }
    };

    if owner_id != user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"success": false, "message": "Not authorized to setup autopay for this application", "error_code": "NOT_OWNER"})),
        );
    }

    // Validate payload conditionally
    match payload.mandate_type {
        crate::models::zero_deposit::MandateType::UpiAutopay => {
            if payload.upi_vpa.is_none() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"success": false, "message": "upi_vpa is required for UPI_AUTOPAY", "error_code": "MISSING_FIELD"})),
                );
            }
        }
        crate::models::zero_deposit::MandateType::Nach => {
            if payload.bank_account_number.is_none() || payload.ifsc.is_none() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"success": false, "message": "bank_account_number and ifsc are required for NACH", "error_code": "MISSING_FIELD"})),
                );
            }
        }
    }

    match crate::services::zero_deposit_service::setup_autopay_mandate(&app_state.db, zero_deposit_id, user_id, &payload).await {
        Ok(Some(mut mandate)) => {
            if let Some(acc) = mandate.bank_account_number.as_ref() {
                mandate.bank_account_number = Some(crate::handlers::zd_wallet_handler::mask_bank_account(acc));
            }
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": "Autopay mandate setup initiated",
                    "data": { "autopay_mandate": mandate }
                })),
            )
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "message": "Zero deposit not found or unauthorized",
                "error_code": "NOT_FOUND"
            })),
        ),
        Err(e) => {
            log::error!("setup_autopay DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Failed to setup autopay",
                    "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

/// GET /api/v1/zero-deposit/{application_id}/repayment-schedule
pub async fn get_repayment_schedule(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<String>,
) -> impl axum::response::IntoResponse {
    let user_id = match require_auth(&headers, &app_state.jwt_secret) {
        Ok(uid) => uid,
        Err((code, body)) => return (code, body),
    };

    let zero_deposit_id = match Uuid::parse_str(&application_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "Invalid zero_deposit_id", "error_code": "INVALID_UUID"})),
            );
        }
    };

    let owner_id = match sqlx::query_scalar::<_, Uuid>(
        "SELECT user_id FROM zero_deposits WHERE id = $1"
    )
    .bind(zero_deposit_id)
    .fetch_optional(&app_state.db)
    .await
    {
        Ok(Some(uid)) => uid,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"success": false, "message": "Application not found", "error_code": "NOT_FOUND"})),
            );
        }
        Err(e) => {
            log::error!("get_repayment_schedule query error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Database error", "error_code": "DB_ERROR"})),
            );
        }
    };

    if owner_id != user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"success": false, "message": "Not authorized to access this repayment schedule", "error_code": "NOT_OWNER"})),
        );
    }

    match crate::services::zero_deposit_service::get_repayment_schedule(&app_state.db, zero_deposit_id, user_id).await {
        Ok(Some(schedule)) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Repayment schedule retrieved successfully",
                "data": { "repayment_schedule": schedule }
            })),
        ),
        Ok(None) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "No repayment schedule found",
                "data": { "repayment_schedule": [] }
            })),
        ),
        Err(e) => {
            log::error!("get_repayment_schedule DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Failed to retrieve repayment schedule",
                    "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

/// GET /api/v1/zero-deposit/{application_id}/ledger
pub async fn get_ledger(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<String>,
) -> impl axum::response::IntoResponse {
    let user_id = match require_auth(&headers, &app_state.jwt_secret) {
        Ok(uid) => uid,
        Err((code, body)) => return (code, body),
    };

    let zero_deposit_id = match Uuid::parse_str(&application_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "Invalid zero_deposit_id", "error_code": "INVALID_UUID"})),
            );
        }
    };

    let owner_id = match sqlx::query_scalar::<_, Uuid>(
        "SELECT user_id FROM zero_deposits WHERE id = $1"
    )
    .bind(zero_deposit_id)
    .fetch_optional(&app_state.db)
    .await
    {
        Ok(Some(uid)) => uid,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"success": false, "message": "Application not found", "error_code": "NOT_FOUND"})),
            );
        }
        Err(e) => {
            log::error!("get_ledger query error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Database error", "error_code": "DB_ERROR"})),
            );
        }
    };

    if owner_id != user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"success": false, "message": "Not authorized to access this ledger", "error_code": "NOT_OWNER"})),
        );
    }

    match crate::services::zero_deposit_service::get_ledger_entries(&app_state.db, zero_deposit_id, user_id).await {
        Ok(Some(entries)) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Ledger retrieved successfully",
                "data": { "ledger": entries }
            })),
        ),
        Ok(None) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "No ledger entries found",
                "data": { "ledger": [] }
            })),
        ),
        Err(e) => {
            log::error!("get_ledger DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Failed to retrieve ledger",
                    "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

/// GET /api/v1/zero-deposit/{zero_deposit_id}/assessment
///
/// Tenant-safe assessment endpoint: NO score, NO raw extracted data, NO fraud flags.
/// Scoped to caller user_id (403 NOT_OWNER if accessed by another user).
pub async fn get_assessment(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(zero_deposit_id): Path<String>,
) -> impl axum::response::IntoResponse {
    let user_id = match require_auth(&headers, &app_state.jwt_secret) {
        Ok(uid) => uid,
        Err((code, body)) => return (code, body),
    };

    let app_uuid = match Uuid::parse_str(&zero_deposit_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "Invalid zero_deposit_id", "error_code": "INVALID_UUID"})),
            );
        }
    };

    // 1. Fetch application and check ownership
    let app_row = sqlx::query(
        "SELECT user_id, status::TEXT as status, created_at, pending_actions FROM zero_deposits WHERE id = $1"
    )
    .bind(app_uuid)
    .fetch_optional(&app_state.db)
    .await;

    let app_row = match app_row {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"success": false, "error": "NOT_FOUND", "message": "Application not found"})),
            );
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "DB_ERROR", "message": e.to_string()})),
            );
        }
    };

    let owner_id: Uuid = app_row.get("user_id");
    if owner_id != user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"success": false, "error": "NOT_OWNER", "message": "You do not own this application"})),
        );
    }

    let app_created_at: chrono::DateTime<chrono::Utc> = app_row.get("created_at");
    let pending_actions: Vec<String> = app_row.try_get("pending_actions").unwrap_or_default();

    // 2. Fetch current assessment
    let assessment_row = sqlx::query(
        r#"SELECT
            id, status::TEXT as status, decision, tier,
            recommended_token_amount::FLOAT8 as tokens_issued,
            tenant_message, reason_codes, completed_at, created_at
        FROM zero_deposit_ai_assessments
        WHERE zero_deposit_id = $1 AND is_current = true
        LIMIT 1"#
    )
    .bind(app_uuid)
    .fetch_optional(&app_state.db)
    .await;

    let assessment_row = match assessment_row {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "DB_ERROR", "message": e.to_string()})),
            );
        }
    };

    let co_applicant_due_by = if pending_actions.iter().any(|a| a == "co_applicant") {
        Some(app_created_at + chrono::Duration::days(7))
    } else {
        None
    };

    match assessment_row {
        Some(r) => {
            let status: String = r.get("status");
            if status == "completed" {
                let decision: Option<String> = r.try_get("decision").ok();
                let tier: Option<String> = r.try_get("tier").ok();
                let tokens_issued: Option<f64> = r.try_get("tokens_issued").ok();
                let tenant_message: Option<String> = r.try_get("tenant_message").ok();
                let completed_at: Option<chrono::DateTime<chrono::Utc>> = r.try_get("completed_at").ok();

                let outcome = match decision.as_deref() {
                    Some("auto_approve") => "tokens_issued",
                    Some("action_required") => "action_required",
                    Some("manual_review") => "under_review",
                    Some("auto_reject") => "rejected",
                    _ => "under_review",
                };

                let reasons = if let Some(msg) = tenant_message {
                    vec![msg]
                } else {
                    vec!["Assessment completed".to_string()]
                };

                let resp = crate::models::zero_deposit::TenantSafeAssessmentResponse {
                    zero_deposit_id: app_uuid,
                    assessment_status: "completed".to_string(),
                    outcome: outcome.to_string(),
                    tier,
                    tokens_issued: if outcome == "tokens_issued" { tokens_issued } else { None },
                    reasons,
                    pending_actions,
                    co_applicant_due_by,
                    assessed_at: completed_at,
                };

                (StatusCode::OK, Json(json!({ "success": true, "data": resp })))
            } else {
                let resp = crate::models::zero_deposit::TenantSafeAssessmentResponse {
                    zero_deposit_id: app_uuid,
                    assessment_status: status,
                    outcome: "in_progress".to_string(),
                    tier: None,
                    tokens_issued: None,
                    reasons: vec!["Assessment is in progress".to_string()],
                    pending_actions,
                    co_applicant_due_by,
                    assessed_at: None,
                };
                (StatusCode::OK, Json(json!({ "success": true, "data": resp })))
            }
        }
        None => {
            let resp = crate::models::zero_deposit::TenantSafeAssessmentResponse {
                zero_deposit_id: app_uuid,
                assessment_status: "queued".to_string(),
                outcome: "in_progress".to_string(),
                tier: None,
                tokens_issued: None,
                reasons: vec!["Assessment is in progress".to_string()],
                pending_actions,
                co_applicant_due_by,
                assessed_at: None,
            };
            (StatusCode::OK, Json(json!({ "success": true, "data": resp })))
        }
    }
}
