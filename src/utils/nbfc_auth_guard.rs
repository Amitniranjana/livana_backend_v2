use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde_json::json;

use crate::{app_state::AppState, models::nbfc_user::NbfcClaims};

pub async fn nbfc_auth_guard(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    let auth_header = req.headers().get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    let token = match auth_header {
        Some(t) => t,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "message": "Missing or invalid authorization token"
                })),
            )
                .into_response());
        }
    };

    // Re-use the existing JWT secret from AppState
    let token_data = decode::<NbfcClaims>(
        &token,
        &DecodingKey::from_secret(state.jwt_secret.as_ref()),
        &Validation::default(),
    );

    match token_data {
        Ok(claims) => {
            // Inject NbfcClaims into request extensions
            let mut req = req;
            req.extensions_mut().insert(claims.claims);
            Ok(next.run(req).await)
        }
        Err(_) => {
            Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "message": "Invalid or expired NBFC session"
                })),
            )
                .into_response())
        }
    }
}
