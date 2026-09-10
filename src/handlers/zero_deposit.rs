use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde_json::json;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    models::zero_deposit::{ApplyZeroDepositRequest, CreditCheckConsentRequest},
    services::zero_deposit_service,
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

fn require_auth(
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
pub async fn apply_zero_deposit(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ApplyZeroDepositRequest>,
) -> impl axum::response::IntoResponse {
    let user_id = match require_auth(&headers, &app_state.jwt_secret) {
        Ok(uid) => uid,
        Err((code, body)) => return (code, body),
    };

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

    match zero_deposit_service::apply_for_zero_deposit(
        &app_state.db,
        user_id,
        property_id,
        kyc_id,
        payload.monthly_rent,
        payload.requested_deposit_amount,
        payload.monthly_income,
        payload.itr_document_url.as_deref(),
        payload.bank_statement_url.as_deref(),
    )
    .await
    {
        Ok(zero_deposit) => (
            StatusCode::CREATED,
            Json(json!({
                "success": true,
                "message": "zero_deposit application submitted successfully",
                "data": { "zero_deposit": zero_deposit }
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
        Ok(Some(zero_deposit)) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "zero_deposit consent updated successfully",
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

    // Hardcode flat fee of 500.0 for MVP as planned
    let fee_amount = 500.0;

    match crate::services::zero_deposit_service::initiate_fee_charge(
        &app_state.db,
        zero_deposit_id,
        user_id,
        &payload.upi_vpa,
        fee_amount,
    )
    .await
    {
        Ok(Some(fee_txn)) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Fee charge initiated successfully",
                "data": { "fee_transaction": fee_txn }
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
