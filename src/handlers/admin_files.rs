use axum::{
    extract::{Query, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Redirect},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

use crate::{app_state::AppState, services::storage::resolve_s3_location};

const ADMIN_VIEW_PRESIGN_SECS: u64 = 10 * 60; // 10 minutes

#[derive(Debug, Deserialize)]
pub struct ViewFileQuery {
    pub url: Option<String>,
}

/// GET /api/admin/files/view?url=<stored url or key>
/// Protected by `admin_auth_guard`. Resolves the S3 bucket and key from the stored
/// URL or key and returns a 307 Temporary Redirect to a real SigV4 presigned GetObject URL.
pub async fn view_file(
    State(state): State<AppState>,
    Query(query): Query<ViewFileQuery>,
) -> impl IntoResponse {
    let raw = query.url.as_deref().unwrap_or("").trim();
    if raw.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": "INVALID_URL",
                "message": "Missing or empty 'url' query parameter"
            })),
        )
            .into_response();
    }

    let (bucket, key) = match resolve_s3_location(raw, state.storage_service.bucket()) {
        Ok(loc) => loc,
        Err(e) => {
            log::warn!("admin_files::view_file rejected url '{}': {}", raw, e);
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false,
                    "error": "INVALID_URL",
                    "message": format!("Invalid S3 URL or key: {}", e)
                })),
            )
                .into_response();
        }
    };

    match state
        .storage_service
        .presigned_get_url(&bucket, &key, Duration::from_secs(ADMIN_VIEW_PRESIGN_SECS))
        .await
    {
        Ok(presigned_url) => (
            [(
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, no-store"),
            )],
            Redirect::temporary(&presigned_url),
        )
            .into_response(),
        Err(e) => {
            log::error!(
                "admin_files::view_file failed to presign bucket={}, key={}: {}",
                bucket,
                key,
                e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "PRESIGN_ERROR",
                    "message": "Failed to generate presigned download URL"
                })),
            )
                .into_response()
        }
    }
}

