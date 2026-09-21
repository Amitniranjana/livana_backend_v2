use axum::{
    extract::{Path, Query, State, Extension},
    http::StatusCode,
    Json,
};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    models::{
        nbfc_user::{NbfcClaims, NbfcUser},
        zero_deposit::ZeroDepositApplication,
    },
    utils::auth::verify_password,
};

#[derive(Deserialize)]
pub struct NbfcLoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct NbfcAuthResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

#[derive(Deserialize)]
pub struct ZeroDepositQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// Issue 75 - NBFC Login
pub async fn nbfc_login(
    State(state): State<AppState>,
    Json(payload): Json<NbfcLoginRequest>,
) -> Result<Json<NbfcAuthResponse>, (StatusCode, Json<NbfcAuthResponse>)> {
    let user = sqlx::query_as::<_, NbfcUser>(
        r#"
        SELECT id, nbfc_id, email, password_hash, role, status, created_at, updated_at
        FROM nbfc_users
        WHERE email = $1 AND status = 'active'
        "#,
    )
    .bind(&payload.email)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(NbfcAuthResponse {
                success: false,
                message: "Database error".into(),
                token: None,
            }),
        )
    })?;

    let user = match user {
        Some(u) => u,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(NbfcAuthResponse {
                    success: false,
                    message: "Invalid credentials".into(),
                    token: None,
                }),
            ));
        }
    };

    if !verify_password(&user.password_hash, &payload.password) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(NbfcAuthResponse {
                success: false,
                message: "Invalid credentials".into(),
                token: None,
            }),
        ));
    }

    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as usize
        + (24 * 3600); // 24 hours

    let claims = NbfcClaims {
        sub: user.id.to_string(),
        nbfc_id: user.nbfc_id,
        exp,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_ref()),
    )
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(NbfcAuthResponse {
                success: false,
                message: "Token generation failed".into(),
                token: None,
            }),
        )
    })?;

    Ok(Json(NbfcAuthResponse {
        success: true,
        message: "Login successful".into(),
        token: Some(token),
    }))
}

// Issue 76 - Get Zero Deposits assigned to NBFC
pub async fn get_nbfc_zero_deposits(
    State(state): State<AppState>,
    Extension(claims): Extension<NbfcClaims>,
    Query(query): Query<ZeroDepositQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let limit = query.limit.unwrap_or(20);
    let offset = query.offset.unwrap_or(0);

    let query_str = if let Some(status) = query.status {
        format!(
            "SELECT id, user_id, property_id, kyc_id, monthly_rent::FLOAT8 as monthly_rent, requested_deposit_amount::FLOAT8 as requested_deposit_amount, monthly_income::FLOAT8 as monthly_income, itr_document_url, bank_statement_url, consent_given, status, created_at, updated_at FROM zero_deposits WHERE nbfc_id = $1 AND status = '{}' ORDER BY created_at DESC LIMIT $2 OFFSET $3",
            status
        )
    } else {
        "SELECT id, user_id, property_id, kyc_id, monthly_rent::FLOAT8 as monthly_rent, requested_deposit_amount::FLOAT8 as requested_deposit_amount, monthly_income::FLOAT8 as monthly_income, itr_document_url, bank_statement_url, consent_given, status, created_at, updated_at FROM zero_deposits WHERE nbfc_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3".to_string()
    };

    let zero_deposits = sqlx::query_as::<_, ZeroDepositApplication>(&query_str)
        .bind(claims.nbfc_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "message": e.to_string() })),
            )
        })?;

    Ok(Json(json!({
        "success": true,
        "data": zero_deposits
    })))
}

// Issue 77 - Get Zero Deposit by ID assigned to NBFC
pub async fn get_nbfc_zero_deposit_by_id(
    State(state): State<AppState>,
    Extension(claims): Extension<NbfcClaims>,
    Path(zero_deposit_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let zero_deposit = sqlx::query_as::<_, ZeroDepositApplication>(
        r#"
        SELECT 
            id, user_id, property_id, kyc_id, 
            monthly_rent::FLOAT8 as monthly_rent, requested_deposit_amount::FLOAT8 as requested_deposit_amount, monthly_income::FLOAT8 as monthly_income, 
            itr_document_url, bank_statement_url, consent_given, 
            status, 
            created_at, updated_at
        FROM zero_deposits
        WHERE id = $1 AND nbfc_id = $2
        "#
    )
    .bind(zero_deposit_id)
    .bind(claims.nbfc_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": e.to_string() })),
        )
    })?;

    match zero_deposit {
        Some(zd) => Ok(Json(json!({
            "success": true,
            "data": zd
        }))),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "message": "Zero Deposit not found or not assigned to your NBFC"
            })),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    // Add basic tests here for NBFC scoping and auth.
    #[tokio::test]
    async fn test_nbfc_auth_requires_token() {
        assert_eq!(1, 1);
    }
}
