use axum::Json;
/// CareCrew Handlers (Step 4)
/// All 8 endpoints for the CareCrew module:
///
///   GET  /api/v1/carecrew/services                        – list all services
///   GET  /api/v1/carecrew/services/{id}                   – service detail
///   GET  /api/v1/carecrew/providers                       – search providers
///   GET  /api/v1/carecrew/providers/featured              – featured providers
///   GET  /api/v1/carecrew/providers/{id}                  – provider detail
///   POST /api/v1/carecrew/bookings                        – create booking (auth)
///   PUT  /api/v1/carecrew/bookings/{id}/status            – update status  (auth)
///   GET  /api/v1/carecrew/providers/{id}/bookings         – provider bookings (auth)
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::models::carecrew::{CreateBookingRequest, UpdateBookingStatusRequest};
use crate::services::carecrew_service::{self, BookingCreateError, BookingUpdateError};

// ─── JWT helper (mirrors the pattern in listing.rs) ───────────────────────────

#[derive(serde::Deserialize, serde::Serialize)]
struct Claims {
    sub: String,
    exp: usize,
}

fn extract_user_id_from_jwt(token: &str, key: &DecodingKey) -> Result<Uuid, String> {
    let data = decode::<Claims>(token, key, &Validation::default()).map_err(|e| e.to_string())?;
    Uuid::parse_str(&data.claims.sub).map_err(|e| e.to_string())
}

fn require_auth(
    headers: &HeaderMap,
    jwt_secret: &str,
) -> Result<Uuid, (StatusCode, axum::Json<serde_json::Value>)> {
    let bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").map(|t| t.to_string()));

    let token = bearer.ok_or_else(|| {
        let body = json!({"success": false, "message": "Missing or invalid Authorization header", "error_code": "UNAUTHORIZED"});
        (StatusCode::UNAUTHORIZED, Json(body))
    })?;

    extract_user_id_from_jwt(&token, &DecodingKey::from_secret(jwt_secret.as_bytes()))
        .map_err(|e| {
            let body = json!({"success": false, "message": format!("Auth error: {}", e), "error_code": "INVALID_TOKEN"});
            (StatusCode::UNAUTHORIZED, Json(body))
        })
}

// ─── Pagination query ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct ProviderSearchQuery {
    #[serde(rename = "serviceType")]
    pub service_type: Option<String>,
    pub city: Option<String>,
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

// ─── 1. List Services ─────────────────────────────────────────────────────────

/// GET /api/v1/carecrew/services
pub async fn list_services(State(app_state): State<AppState>) -> impl axum::response::IntoResponse {
    match carecrew_service::list_services(&app_state.db).await {
        Ok(data) => {
            let body = json!({
                "success": true,
                "message": "CareCrew services retrieved successfully",
                "data": data
            });
            (StatusCode::OK, Json(body))
        }
        Err(e) => {
            log::error!("CareCrew list_services error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Failed to retrieve services",
                    "error_code": "DB_ERROR",
                    "errors": [e.to_string()]
                })),
            )
        }
    }
}

// ─── 2. Get Service by ID ─────────────────────────────────────────────────────

/// GET /api/v1/carecrew/services/{id}
pub async fn get_service(
    State(app_state): State<AppState>,
    Path(id): Path<String>,
) -> impl axum::response::IntoResponse {
    let service_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false, "message": "Invalid service ID", "error_code": "INVALID_UUID"
                })),
            );
        }
    };

    match carecrew_service::get_service_by_id(&app_state.db, service_id).await {
        Ok(Some(service)) => (
            StatusCode::OK,
            Json(json!({
                "success": true, "message": "Service retrieved successfully",
                "data": { "service": service }
            })),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false, "message": "Service not found", "error_code": "NOT_FOUND"
            })),
        ),
        Err(e) => {
            log::error!("get_service DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false, "message": "Database error", "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

// ─── 3. Search Providers ──────────────────────────────────────────────────────

/// GET /api/v1/carecrew/providers?serviceType=plumbing&city=Mumbai&page=1&limit=10
pub async fn search_providers(
    State(app_state): State<AppState>,
    Query(q): Query<ProviderSearchQuery>,
) -> impl axum::response::IntoResponse {
    let page = q.page.unwrap_or(1).max(1);
    let limit = q.limit.unwrap_or(10).clamp(1, 50);

    match carecrew_service::search_providers(
        &app_state.db,
        q.service_type.as_deref(),
        q.city.as_deref(),
        page,
        limit,
    )
    .await
    {
        Ok(result) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Providers retrieved successfully",
                "data": {
                    "providers": result.providers,
                    "pagination": {
                        "total_count": result.total_count,
                        "current_page": result.current_page,
                        "total_pages": result.total_pages,
                        "limit": limit
                    }
                }
            })),
        ),
        Err(e) => {
            log::error!("search_providers DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false, "message": "Database error", "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

// ─── 4. Featured Providers ────────────────────────────────────────────────────

/// GET /api/v1/carecrew/providers/featured
pub async fn get_featured_providers(
    State(app_state): State<AppState>,
) -> impl axum::response::IntoResponse {
    match carecrew_service::get_featured_providers(&app_state.db, 10).await {
        Ok(data) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Featured providers retrieved successfully",
                "data": data
            })),
        ),
        Err(e) => {
            log::error!("get_featured_providers DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false, "message": "Database error", "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

// ─── 5. Get Provider by ID ────────────────────────────────────────────────────

/// GET /api/v1/carecrew/providers/{id}
pub async fn get_provider(
    State(app_state): State<AppState>,
    Path(id): Path<String>,
) -> impl axum::response::IntoResponse {
    let provider_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false, "message": "Invalid provider ID", "error_code": "INVALID_UUID"
                })),
            );
        }
    };

    match carecrew_service::get_provider_by_id(&app_state.db, provider_id).await {
        Ok(Some(provider)) => (
            StatusCode::OK,
            Json(json!({
                "success": true, "message": "Provider retrieved successfully",
                "data": { "provider": provider }
            })),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false, "message": "Provider not found", "error_code": "NOT_FOUND"
            })),
        ),
        Err(e) => {
            log::error!("get_provider DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false, "message": "Database error", "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

// ─── 6. Create Booking (Auth) ─────────────────────────────────────────────────

/// POST /api/v1/carecrew/bookings
/// Requires: Authorization: Bearer <token>
pub async fn create_booking(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateBookingRequest>,
) -> impl axum::response::IntoResponse {
    let user_id = match require_auth(&headers, &app_state.jwt_secret) {
        Ok(uid) => uid,
        Err((code, body)) => return (code, body),
    };

    // Validate UUIDs from payload
    let provider_id = match Uuid::parse_str(&payload.provider_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false, "message": "Invalid provider_id", "error_code": "INVALID_UUID"
                })),
            );
        }
    };
    let service_id = match Uuid::parse_str(&payload.service_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false, "message": "Invalid service_id", "error_code": "INVALID_UUID"
                })),
            );
        }
    };

    match carecrew_service::create_booking(
        &app_state.db,
        provider_id,
        service_id,
        user_id,
        &payload.scheduled_at,
        payload.notes.as_deref(),
    )
    .await
    {
        Ok(booking) => (
            StatusCode::CREATED,
            Json(json!({
                "success": true, "message": "Booking created successfully",
                "data": { "booking": booking }
            })),
        ),
        Err(BookingCreateError::ProviderNotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false, "message": "Provider not found", "error_code": "PROVIDER_NOT_FOUND"
            })),
        ),
        Err(BookingCreateError::ServiceNotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false, "message": "Service not found", "error_code": "SERVICE_NOT_FOUND"
            })),
        ),
        Err(BookingCreateError::InvalidScheduledAt) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "message": "Invalid scheduled_at format — must be ISO 8601 e.g. 2026-03-01T10:00:00Z",
                "error_code": "INVALID_DATETIME"
            })),
        ),
        Err(BookingCreateError::DbError(e)) => {
            log::error!("create_booking DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false, "message": "Database error", "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

// ─── 7. Update Booking Status (Auth) ─────────────────────────────────────────

/// PUT /api/v1/carecrew/bookings/{id}/status
/// Requires: Authorization: Bearer <token>
pub async fn update_booking_status(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<UpdateBookingStatusRequest>,
) -> impl axum::response::IntoResponse {
    // Auth check (any authenticated user may update — e.g. provider or admin side)
    let _user_id = match require_auth(&headers, &app_state.jwt_secret) {
        Ok(uid) => uid,
        Err((code, body)) => return (code, body),
    };

    let booking_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false, "message": "Invalid booking ID", "error_code": "INVALID_UUID"
                })),
            );
        }
    };

    match carecrew_service::update_booking_status(&app_state.db, booking_id, &payload.status).await
    {
        Ok(booking) => (
            StatusCode::OK,
            Json(json!({
                "success": true, "message": "Booking status updated successfully",
                "data": { "booking": booking }
            })),
        ),
        Err(BookingUpdateError::BookingNotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false, "message": "Booking not found", "error_code": "NOT_FOUND"
            })),
        ),
        Err(BookingUpdateError::InvalidStatus(s)) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "message": format!("'{}' is not a valid status. Allowed: pending, confirmed, in_progress, completed, cancelled", s),
                "error_code": "INVALID_STATUS"
            })),
        ),
        Err(BookingUpdateError::InvalidTransition { from, to }) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "success": false,
                "message": format!("Cannot transition booking from '{}' to '{}'", from, to),
                "error_code": "INVALID_TRANSITION"
            })),
        ),
        Err(BookingUpdateError::DbError(e)) => {
            log::error!("update_booking_status DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false, "message": "Database error", "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

// ─── 8. Get Provider Bookings (Auth) ─────────────────────────────────────────

/// GET /api/v1/carecrew/providers/{id}/bookings
/// Requires: Authorization: Bearer <token>
pub async fn get_provider_bookings(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<PaginationQuery>,
) -> impl axum::response::IntoResponse {
    let _user_id = match require_auth(&headers, &app_state.jwt_secret) {
        Ok(uid) => uid,
        Err((code, body)) => return (code, body),
    };

    let provider_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false, "message": "Invalid provider ID", "error_code": "INVALID_UUID"
                })),
            );
        }
    };

    let page = q.page.unwrap_or(1).max(1);
    let limit = q.limit.unwrap_or(10).clamp(1, 50);

    match carecrew_service::get_provider_bookings(&app_state.db, provider_id, page, limit).await {
        Ok(result) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Bookings retrieved successfully",
                "data": {
                    "bookings": result.providers, // using providers field for bookings list
                    "pagination": {
                        "total_count": result.total_count,
                        "current_page": result.current_page,
                        "total_pages": result.total_pages,
                        "limit": limit
                    }
                }
            })),
        ),
        Err(e) => {
            log::error!("get_provider_bookings DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false, "message": "Database error", "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::models::carecrew::{
        CreateBookingRequest, UpdateBookingStatusRequest, is_valid_status, is_valid_transition,
    };

    #[test]
    fn test_provider_pagination_offset() {
        // page=2, limit=5 → offset=5, page=3 limit=5 → offset=10
        let page2_limit5 = (2i32 - 1) * 5i32;
        assert_eq!(page2_limit5, 5);
        let page3_limit5 = (3i32 - 1) * 5i32;
        assert_eq!(page3_limit5, 10);
    }

    #[test]
    fn test_limit_clamped() {
        // limit must be 1..=50 for providers
        let too_large = 200i32.clamp(1, 50);
        assert_eq!(too_large, 50);
        let zero = 0i32.clamp(1, 50);
        assert_eq!(zero, 1);
    }

    #[test]
    fn test_total_pages_math() {
        // 37 items, limit=10 → 4 pages
        let total = 37i64;
        let limit = 10i32;
        let pages = ((total as f64) / (limit as f64)).ceil() as i32;
        assert_eq!(pages, 4);
    }

    #[test]
    fn test_uuid_parse_valid() {
        let id = "550e8400-e29b-41d4-a716-446655440000";
        assert!(uuid::Uuid::parse_str(id).is_ok());
    }

    #[test]
    fn test_uuid_parse_invalid() {
        let id = "not-a-valid-uuid";
        assert!(uuid::Uuid::parse_str(id).is_err());
    }

    #[test]
    fn test_status_valid_values() {
        assert!(is_valid_status("pending"));
        assert!(is_valid_status("confirmed"));
        assert!(is_valid_status("in_progress"));
        assert!(is_valid_status("completed"));
        assert!(is_valid_status("cancelled"));
    }

    #[test]
    fn test_status_invalid_values() {
        assert!(!is_valid_status("active"));
        assert!(!is_valid_status("unknown"));
        assert!(!is_valid_status(""));
    }

    #[test]
    fn test_terminal_states_no_transition() {
        // completed and cancelled are terminal — no outbound transitions
        assert!(!is_valid_transition("completed", "pending"));
        assert!(!is_valid_transition("completed", "confirmed"));
        assert!(!is_valid_transition("cancelled", "pending"));
        assert!(!is_valid_transition("cancelled", "confirmed"));
    }

    #[test]
    fn test_forward_transitions_valid() {
        assert!(is_valid_transition("pending", "confirmed"));
        assert!(is_valid_transition("confirmed", "in_progress"));
        assert!(is_valid_transition("in_progress", "completed"));
    }

    #[test]
    fn test_skipping_transitions_invalid() {
        // Cannot jump from pending → completed (must go through confirmed, in_progress)
        assert!(!is_valid_transition("pending", "completed"));
        assert!(!is_valid_transition("pending", "in_progress"));
    }

    #[test]
    fn test_create_booking_request_deserialization() {
        let json_str = r#"{
            "provider_id": "550e8400-e29b-41d4-a716-446655440000",
            "service_id":  "550e8400-e29b-41d4-a716-446655440001",
            "scheduled_at": "2026-03-01T10:00:00Z",
            "notes": "Please arrive by 10am"
        }"#;
        let req: CreateBookingRequest = serde_json::from_str(json_str).unwrap();
        assert!(!req.provider_id.is_empty());
        assert!(!req.service_id.is_empty());
        assert_eq!(req.notes.as_deref(), Some("Please arrive by 10am"));
    }

    #[test]
    fn test_update_status_deserialization() {
        let json_str = r#"{"status": "confirmed"}"#;
        let req: UpdateBookingStatusRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(req.status, "confirmed");
    }

    #[test]
    fn test_scheduled_at_iso8601_valid() {
        let valid = "2026-03-01T10:00:00Z";
        assert!(chrono::DateTime::parse_from_rfc3339(valid).is_ok());
    }

    #[test]
    fn test_scheduled_at_iso8601_invalid() {
        let invalid = "01-03-2026 10:00";
        assert!(chrono::DateTime::parse_from_rfc3339(invalid).is_err());
    }
}
