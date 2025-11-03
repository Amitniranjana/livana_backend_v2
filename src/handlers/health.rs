use axum::{
    http::StatusCode,
    response::Json,
};
use serde_json::json;

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Server is running", body = ApiResponse<serde_json::Value>)
    ),
    tag = "Health"
)]
pub async fn get_health() -> impl axum::response::IntoResponse {
    let response = json!({
        "success": true,
        "message": "Server is running",
        "data": {
            "status": "healthy",
            "timestamp": "2024-01-15T21:00:00Z",
            "version": "1.0.0",
            "uptime": "2 hours 30 minutes"
        }
    });
    
    (StatusCode::OK, Json(response))
}