use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
    Json,
};
use axum_extra::extract::cookie::CookieJar;
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde_json::json;

use crate::{app_state::AppState, handlers::admin_auth::AdminClaims};

pub async fn admin_auth_guard(
    State(state): State<AppState>,
    jar: CookieJar,
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    let session_cookie = jar.get("admin_session").map(|c| c.value().to_string());
    
    let auth_header = req.headers().get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    let is_api = req.uri().path().starts_with("/api/");

    let token = match session_cookie.or(auth_header) {
        Some(t) => t,
        None => {
            if is_api {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(json!({
                        "success": false,
                        "message": "No active admin session"
                    })),
                )
                    .into_response());
            } else {
                return Err(Redirect::to("/admin/login").into_response());
            }
        }
    };

    let token_data = decode::<AdminClaims>(
        &token,
        &DecodingKey::from_secret(state.admin_jwt_secret.as_ref()),
        &Validation::default(),
    );

    match token_data {
        Ok(claims) => {
            // Optional: you can inject claims into request extensions
            let mut req = req;
            req.extensions_mut().insert(claims.claims);
            Ok(next.run(req).await)
        }
        Err(_) => {
            if is_api {
                Err((
                    StatusCode::UNAUTHORIZED,
                    Json(json!({
                        "success": false,
                        "message": "Invalid or expired admin session"
                    })),
                )
                    .into_response())
            } else {
                Err(Redirect::to("/admin/login").into_response())
            }
        }
    }
}
