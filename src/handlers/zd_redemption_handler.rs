//! Issue 116, 117, 118, 119 & ZD v2 Spec — Tenant Redemption Chat Handlers
//!
//! Provides:
//! - POST /api/v1/zero-deposit/{zero_deposit_id}/redemptions (Header Idempotency-Key required)
//! - GET /api/v1/zero-deposit/redemptions/me
//! - GET /api/v1/zero-deposit/redemptions/{redemption_id}
//! - GET /api/v1/zero-deposit/redemptions/{redemption_id}/messages
//! - POST /api/v1/zero-deposit/redemptions/{redemption_id}/messages
//! - POST /api/v1/zero-deposit/redemptions/{redemption_id}/documents
//! - POST /api/v1/zero-deposit/redemptions/{redemption_id}/submit
//! - POST /api/v1/zero-deposit/redemptions/{redemption_id}/esign
//! - POST /api/v1/zero-deposit/redemptions/{redemption_id}/cancel

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use axum_extra::extract::Multipart;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    handlers::zero_deposit::require_auth,
    services::zd_redemption_service::{
        self, CancelRedemptionReq, OpenRedemptionReq, RedemptionError, SendMessageReq,
        SubmitRedemptionReq,
    },
};

#[derive(Debug, Deserialize)]
pub struct RedemptionsMeQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct MessagesQuery {
    pub before: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
}

pub fn map_redemption_error(err: RedemptionError) -> (StatusCode, Json<serde_json::Value>) {
    match err {
        RedemptionError::DuplicateRequest => (
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "DUPLICATE_REQUEST",
                "message": "Idempotency key reused with different request payload."
            })),
        ),
        RedemptionError::RedemptionAlreadyOpen => (
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "REDEMPTION_ALREADY_OPEN",
                "message": "An active redemption is already open for this application."
            })),
        ),
        RedemptionError::TokensExpired => (
            StatusCode::GONE,
            Json(json!({
                "success": false,
                "error": "TOKENS_EXPIRED",
                "message": "Tokens have expired or allocation is inactive."
            })),
        ),
        RedemptionError::InsufficientTokens { requested, available } => (
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "INSUFFICIENT_TOKENS",
                "message": format!("Requested tokens ({}) exceed available balance ({}).", requested, available)
            })),
        ),
        RedemptionError::DocumentsMissing(docs) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "success": false,
                "error": "DOCUMENTS_MISSING",
                "message": "Required documents are missing before submission.",
                "missing_documents": docs
            })),
        ),
        RedemptionError::FileTooLarge => (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({
                "success": false,
                "error": "FILE_TOO_LARGE",
                "message": "File exceeds the 10 MB maximum upload limit."
            })),
        ),
        RedemptionError::UnsupportedFileType => (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Json(json!({
                "success": false,
                "error": "UNSUPPORTED_FILE_TYPE",
                "message": "Unsupported file type. Only PDF, JPG, and PNG are allowed."
            })),
        ),
        RedemptionError::PreconditionFailed(keys) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "success": false,
                "error": "PRECONDITION_FAILED",
                "message": "Preconditions not satisfied.",
                "failed_preconditions": keys
            })),
        ),
        RedemptionError::MakerCheckerViolation => (
            StatusCode::FORBIDDEN,
            Json(json!({
                "success": false,
                "error": "MAKER_CHECKER",
                "message": "Releasing admin must differ from approving admin."
            })),
        ),
        RedemptionError::AmountMismatch => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "success": false,
                "error": "AMOUNT_MISMATCH",
                "message": "Released amount does not match approved amount."
            })),
        ),
        RedemptionError::DuplicateUtr => (
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "DUPLICATE_UTR",
                "message": "Duplicate UTR reference detected."
            })),
        ),
        RedemptionError::InvalidState(msg) => (
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "INVALID_STATE",
                "message": msg
            })),
        ),
        RedemptionError::NotOwner => (
            StatusCode::FORBIDDEN,
            Json(json!({
                "success": false,
                "error": "NOT_OWNER",
                "message": "Not authorized to access this redemption."
            })),
        ),
        RedemptionError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": "NOT_FOUND",
                "message": "Redemption or application not found."
            })),
        ),
        RedemptionError::Db(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": "DB_ERROR", "message": e.to_string() })),
        ),
        RedemptionError::Ledger(e) => (
            StatusCode::CONFLICT,
            Json(json!({ "success": false, "error": "LEDGER_ERROR", "message": e.to_string() })),
        ),
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/v1/zero-deposit/{zero_deposit_id}/redemptions
pub async fn open_redemption_chat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(zero_deposit_id): Path<Uuid>,
    Json(payload): Json<OpenRedemptionReq>,
) -> impl IntoResponse {
    let user_id = match require_auth(&headers, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => return e.into_response(),
    };

    let idempotency_header = match headers.get("idempotency-key").and_then(|v| v.to_str().ok()) {
        Some(k) => match Uuid::parse_str(k) {
            Ok(u) => u,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "success": false,
                        "error": "INVALID_IDEMPOTENCY_KEY",
                        "message": "Idempotency-Key must be a valid UUID"
                    })),
                )
                    .into_response();
            }
        },
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false,
                    "error": "MISSING_IDEMPOTENCY_KEY",
                    "message": "Idempotency-Key header is required"
                })),
            )
                .into_response();
        }
    };

    match zd_redemption_service::open_redemption_chat(
        &state,
        zero_deposit_id,
        user_id,
        idempotency_header,
        &payload,
    )
    .await
    {
        Ok(resp) => (StatusCode::CREATED, Json(json!({ "success": true, "data": resp }))).into_response(),
        Err(e) => map_redemption_error(e).into_response(),
    }
}

/// GET /api/v1/zero-deposit/redemptions/me
pub async fn get_my_redemptions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<RedemptionsMeQuery>,
) -> impl IntoResponse {
    let user_id = match require_auth(&headers, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => return e.into_response(),
    };

    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);

    match zd_redemption_service::get_my_redemptions(
        &state.db,
        user_id,
        query.status.as_deref(),
        limit,
        offset,
    )
    .await
    {
        Ok(redemptions) => (
            StatusCode::OK,
            Json(json!({ "success": true, "data": { "redemptions": redemptions } })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// GET /api/v1/zero-deposit/redemptions/{redemption_id}
pub async fn get_redemption_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(redemption_id): Path<Uuid>,
) -> impl IntoResponse {
    let user_id = match require_auth(&headers, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => return e.into_response(),
    };

    match zd_redemption_service::get_redemption_detail_tenant(&state.db, user_id, redemption_id).await {
        Ok(detail) => (StatusCode::OK, Json(json!({ "success": true, "data": detail }))).into_response(),
        Err(e) => map_redemption_error(e).into_response(),
    }
}

/// GET /api/v1/zero-deposit/redemptions/{redemption_id}/messages
pub async fn get_redemption_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(redemption_id): Path<Uuid>,
    Query(query): Query<MessagesQuery>,
) -> impl IntoResponse {
    let user_id = match require_auth(&headers, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => return e.into_response(),
    };

    // Scoped ownership check
    let owner_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM zd_redemptions WHERE id = $1"
    )
    .bind(redemption_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    if owner_id != Some(user_id) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": "NOT_OWNER", "message": "Not authorized" })),
        )
            .into_response();
    }

    let limit = query.limit.unwrap_or(50).min(100);

    match zd_redemption_service::get_redemption_messages(&state.db, redemption_id, query.before, limit).await {
        Ok(messages) => (
            StatusCode::OK,
            Json(json!({ "success": true, "data": { "messages": messages } })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/v1/zero-deposit/redemptions/{redemption_id}/messages
pub async fn send_redemption_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(redemption_id): Path<Uuid>,
    Json(payload): Json<SendMessageReq>,
) -> impl IntoResponse {
    let user_id = match require_auth(&headers, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => return e.into_response(),
    };

    match zd_redemption_service::send_redemption_message_tenant(&state, user_id, redemption_id, &payload.text).await {
        Ok(msg) => (StatusCode::CREATED, Json(json!({ "success": true, "data": msg }))).into_response(),
        Err(e) => map_redemption_error(e).into_response(),
    }
}

/// POST /api/v1/zero-deposit/redemptions/{redemption_id}/documents
pub async fn upload_redemption_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(redemption_id): Path<Uuid>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let user_id = match require_auth(&headers, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => return e.into_response(),
    };

    let mut doc_type: Option<String> = None;
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut original_filename: Option<String> = None;
    let mut note: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "doc_type" {
            doc_type = field.text().await.ok();
        } else if name == "note" {
            note = field.text().await.ok();
        } else if name == "file" {
            original_filename = field.file_name().map(|s| s.to_string());
            file_bytes = field.bytes().await.ok().map(|b| b.to_vec());
        }
    }

    let doc_type = match doc_type {
        Some(dt) => dt,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": "MISSING_FIELD", "message": "doc_type is required" })),
            )
                .into_response();
        }
    };

    let file_bytes = match file_bytes {
        Some(fb) => fb,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": "MISSING_FIELD", "message": "file is required" })),
            )
                .into_response();
        }
    };

    match zd_redemption_service::upload_redemption_document(
        &state,
        user_id,
        redemption_id,
        &doc_type,
        file_bytes,
        original_filename.as_deref(),
        note.as_deref(),
    )
    .await
    {
        Ok(res) => (StatusCode::CREATED, Json(json!({ "success": true, "data": res }))).into_response(),
        Err(e) => map_redemption_error(e).into_response(),
    }
}

/// POST /api/v1/zero-deposit/redemptions/{redemption_id}/submit
pub async fn submit_redemption(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(redemption_id): Path<Uuid>,
    Json(payload): Json<SubmitRedemptionReq>,
) -> impl IntoResponse {
    let user_id = match require_auth(&headers, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => return e.into_response(),
    };

    match zd_redemption_service::submit_redemption(&state, user_id, redemption_id, &payload).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(e) => map_redemption_error(e).into_response(),
    }
}

/// POST /api/v1/zero-deposit/redemptions/{redemption_id}/esign
pub async fn get_esign_link(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(redemption_id): Path<Uuid>,
) -> impl IntoResponse {
    let user_id = match require_auth(&headers, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => return e.into_response(),
    };

    match zd_redemption_service::get_esign_link(&state.db, user_id, redemption_id).await {
        Ok(res) => (StatusCode::OK, Json(json!({ "success": true, "data": res }))).into_response(),
        Err(e) => map_redemption_error(e).into_response(),
    }
}

/// POST /api/v1/zero-deposit/redemptions/{redemption_id}/cancel
pub async fn cancel_redemption(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(redemption_id): Path<Uuid>,
    Json(payload): Json<CancelRedemptionReq>,
) -> impl IntoResponse {
    let user_id = match require_auth(&headers, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => return e.into_response(),
    };

    match zd_redemption_service::cancel_redemption(&state, user_id, redemption_id, &payload).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(e) => map_redemption_error(e).into_response(),
    }
}
