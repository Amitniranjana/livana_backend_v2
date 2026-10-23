/// Issue 97 — Velocity-check middleware for POST /api/v1/loans/apply
///
/// This middleware runs *before* the apply handler and enforces sliding-window
/// rate limits keyed on:
///   • `X-Device-Id` header  → at most DEVICE_LIMIT per DEVICE_WINDOW_SECS
///   • peer IP address       → at most IP_LIMIT per IP_WINDOW_SECS
///
/// Counters are tracked in Redis (fast path) with a Postgres fallback.
/// The middleware also extracts the device_id and client_ip and injects them
/// into request extensions so the apply handler can persist them without
/// re-parsing headers.

use axum::{
    body::Body,
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::net::SocketAddr;

use crate::{
    app_state::AppState,
    services::fraud_service::{check_velocity, DEVICE_LIMIT, IP_LIMIT, DEVICE_WINDOW_SECS, IP_WINDOW_SECS},
};

/// Contextual metadata injected by this middleware into request extensions.
#[derive(Debug, Clone)]
pub struct LoanApplyContext {
    /// Value of the `X-Device-Id` header, if present.
    pub device_id: Option<String>,
    /// Peer IP address as a string (e.g. "203.0.113.45").
    pub client_ip: Option<String>,
}

pub async fn loan_velocity_guard(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, Response> {
    // ── Extract device_id from X-Device-Id header ────────────────────────
    let device_id = request
        .headers()
        .get("x-device-id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    // ── Resolve client IP (prefer X-Forwarded-For behind load balancer) ──
    let forwarded_ip = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next()) // first entry is the real client IP
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().to_string());

    let client_ip = forwarded_ip.or_else(|| Some(addr.ip().to_string()));

    // ── Velocity check ───────────────────────────────────────────────────
    if let Some(reject_reason) = check_velocity(
        &state.db,
        &state.redis_pool,
        device_id.as_deref(),
        client_ip.as_deref(),
    )
    .await
    {
        let body = json!({
            "success": false,
            "message": reject_reason,
            "error_code": "VELOCITY_LIMIT_EXCEEDED",
            "limits": {
                "device": {
                    "max_applications": DEVICE_LIMIT,
                    "window_seconds": DEVICE_WINDOW_SECS
                },
                "ip": {
                    "max_applications": IP_LIMIT,
                    "window_seconds": IP_WINDOW_SECS
                }
            }
        });
        return Err((StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response());
    }

    // ── Inject context into extensions for the apply handler ─────────────
    request
        .extensions_mut()
        .insert(LoanApplyContext { device_id, client_ip });

    Ok(next.run(request).await)
}
