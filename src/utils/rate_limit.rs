use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use redis::AsyncCommands;
use std::net::SocketAddr;

use crate::app_state::AppState;
use crate::handlers::admin_auth::AdminClaims;
use crate::utils::api_error::ApiErrorResponse;

pub async fn login_rate_limiter(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    if let Some(mut redis_pool) = state.redis_pool.clone() {
        let ip = addr.ip().to_string();
        let key = format!("rate_limit:login:{}", ip);
        
        // 5 attempts / 15 minutes
        let count: i32 = redis_pool.incr(&key, 1).await.unwrap_or(1);
        if count == 1 {
            let _: () = redis_pool.expire(&key, 15 * 60).await.unwrap_or(());
        }
        
        if count > 5 {
            let err = ApiErrorResponse {
                success: false,
                message: "Too many login attempts. Please try again later.".to_string(),
                error_code: "RATE_LIMIT_EXCEEDED".to_string(),
                errors: None,
            };
            return Err((StatusCode::TOO_MANY_REQUESTS, Json(err)).into_response());
        }
    }
    
    Ok(next.run(request).await)
}

pub async fn force_delete_rate_limiter(
    State(state): State<AppState>,
    claims: axum::extract::Extension<AdminClaims>,
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    if let Some(mut redis_pool) = state.redis_pool.clone() {
        let user_id = &claims.sub;
        let key = format!("rate_limit:force_delete:{}", user_id);
        
        // 20 attempts / 1 minute
        let count: i32 = redis_pool.incr(&key, 1).await.unwrap_or(1);
        if count == 1 {
            let _: () = redis_pool.expire(&key, 60).await.unwrap_or(());
        }
        
        if count > 20 {
            let err = ApiErrorResponse {
                success: false,
                message: "Too many force delete attempts. Please wait 1 minute.".to_string(),
                error_code: "RATE_LIMIT_EXCEEDED".to_string(),
                errors: None,
            };
            return Err((StatusCode::TOO_MANY_REQUESTS, Json(err)).into_response());
        }
    }
    
    Ok(next.run(request).await)
}
