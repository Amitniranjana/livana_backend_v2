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
    models::loan::{ApplyLoanRequest, CreditCheckConsentRequest},
    services::loan_service,
};

#[derive(serde::Deserialize)]
struct Claims {
    sub: String,
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

/// POST /api/v1/loans/apply
pub async fn apply_loan(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ApplyLoanRequest>,
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

    match loan_service::apply_for_loan(
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
        Ok(loan) => (
            StatusCode::CREATED,
            Json(json!({
                "success": true,
                "message": "Loan application submitted successfully",
                "data": { "loan": loan }
            })),
        ),
        Err(e) => {
            log::error!("apply_loan DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Failed to submit loan application",
                    "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

/// POST /api/v1/loans/credit-check/consent
pub async fn credit_check_consent(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreditCheckConsentRequest>,
) -> impl axum::response::IntoResponse {
    let user_id = match require_auth(&headers, &app_state.jwt_secret) {
        Ok(uid) => uid,
        Err((code, body)) => return (code, body),
    };

    let loan_id = match Uuid::parse_str(&payload.loan_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "Invalid loan_id", "error_code": "INVALID_UUID"})),
            );
        }
    };

    match loan_service::update_consent(&app_state.db, loan_id, user_id, payload.consent_given).await {
        Ok(Some(loan)) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Loan consent updated successfully",
                "data": { "loan": loan }
            })),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "message": "Loan not found or unauthorized",
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

/// GET /api/v1/loans/{loan_id}
pub async fn get_loan(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(loan_id): Path<String>,
) -> impl axum::response::IntoResponse {
    let user_id = match require_auth(&headers, &app_state.jwt_secret) {
        Ok(uid) => uid,
        Err((code, body)) => return (code, body),
    };

    let loan_id = match Uuid::parse_str(&loan_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "Invalid loan_id", "error_code": "INVALID_UUID"})),
            );
        }
    };

    match loan_service::get_loan_by_id(&app_state.db, loan_id, user_id).await {
        Ok(Some(loan)) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Loan retrieved successfully",
                "data": { "loan": loan }
            })),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "message": "Loan not found or unauthorized",
                "error_code": "NOT_FOUND"
            })),
        ),
        Err(e) => {
            log::error!("get_loan DB error: {}", e);
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
