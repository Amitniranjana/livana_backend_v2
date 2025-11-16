use axum::{extract::Extension, Json, response::IntoResponse, http::StatusCode};
use crate::dtos::auth::{SignupRequest, SigninRequest, AuthResponse};
use crate::app_state::AppState;
use crate::services::auth_service::AuthService;
use serde_json::json;

pub async fn signup(
    Extension(state): Extension<AppState>,
    Json(payload): Json<SignupRequest>,
) -> impl IntoResponse {
    let res = AuthService::register(
        &state.db,
        payload.first_name.clone(),
        payload.last_name.clone(),
        payload.email.clone(),
        payload.password.clone(),
        payload.phone_no.clone(),
        payload.gender.clone(),
        payload.user_role.clone(),
        payload.business_name.clone(),
        payload.license_number.clone(),
        payload.experience_years,
        payload.commission_rate,
        &state.jwt_secret,
    ).await;

    match res {
        Ok((user, token)) => {
            let resp = AuthResponse {
                token,
                user_id: user.id.to_string(),
                email: user.email,
                first_name: user.first_name,
                last_name: user.last_name,
            };
            (StatusCode::CREATED, Json(resp))
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()}))),
    }
}

pub async fn signin(
    Extension(state): Extension<AppState>,
    Json(payload): Json<SigninRequest>,
) -> impl IntoResponse {
    match AuthService::login(&state.db, payload.email.clone(), payload.password.clone(), &state.jwt_secret).await {
        Ok((user, token)) => {
            let resp = AuthResponse {
                token,
                user_id: user.id.to_string(),
                email: user.email,
                first_name: user.first_name,
                last_name: user.last_name,
            };
            (StatusCode::OK, Json(resp))
        }
        Err(e) => (StatusCode::UNAUTHORIZED, Json(json!({"error": e.to_string()}))),
    }
}
