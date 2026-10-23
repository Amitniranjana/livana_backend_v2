/// Issue 96 — GET /api/admin/loans/fraud-signals
///
/// Admin dashboard view: lists all zero_deposit applications that have been
/// flagged with one or more fraud signals by the async detection job.
/// Also exposes a summary widget endpoint.

use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    services::fraud_service::{
        list_fraud_flagged_applications,
        fraud_signal_summary,
        run_batch_fraud_scan,
        DEVICE_LIMIT, IP_LIMIT, DEVICE_WINDOW_SECS, IP_WINDOW_SECS,
    },
};

// ─── GET /api/admin/loans/fraud-signals ─────────────────────────────────────

/// Returns all applications flagged by the fraud-detection job.
///
/// Response shape:
/// ```json
/// {
///   "success": true,
///   "data": {
///     "summary": { "total_flagged_applications": 12, "by_code": [...] },
///     "applications": [...]
///   }
/// }
/// ```
pub async fn get_fraud_signals(
    State(state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let applications = list_fraud_flagged_applications(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "message": e })),
            )
        })?;

    let summary = fraud_signal_summary(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "message": e })),
            )
        })?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "summary": summary,
            "applications": applications
        }
    })))
}

// ─── GET /api/admin/loans/velocity-config ────────────────────────────────────

/// Returns the current velocity-check configuration so the admin UI can display
/// the active thresholds without hard-coding them in the frontend.
pub async fn get_velocity_config(
    State(_state): State<AppState>,
) -> Json<Value> {
    Json(json!({
        "success": true,
        "data": {
            "device": {
                "max_applications": DEVICE_LIMIT,
                "window_seconds": DEVICE_WINDOW_SECS,
                "window_human": "1 hour"
            },
            "ip": {
                "max_applications": IP_LIMIT,
                "window_seconds": IP_WINDOW_SECS,
                "window_human": "1 hour"
            },
            "note": "When Redis is available counters are tracked in-memory with TTL. When Redis is absent, Postgres loan_apply_events is used as a sliding-window counter."
        }
    }))
}

// ─── POST /api/admin/loans/fraud-signals/scan ────────────────────────────────

/// Triggers a batch anomaly-detection scan across all applications from the last 90 days.
/// Can be invoked on-demand by admins or scheduled as a nightly cron job.
pub async fn trigger_batch_fraud_scan(
    State(state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = run_batch_fraud_scan(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "message": e })),
            )
        })?;

    Ok(Json(json!({
        "success": true,
        "message": "Batch anomaly detection scan completed successfully",
        "data": result
    })))
}

