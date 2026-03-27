/// CareCrew Tickets Handlers
/// 5 authenticated Axum endpoints for the ticket lifecycle.
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::models::carecrew::{AddTicketCommentRequest, CreateTicketRequest, UpdateTicketRequest};
use crate::services::carecrew_tickets_service::{self as svc, TicketError};

// ─── JWT extraction helper (same pattern as carecrew.rs) ─────────────────────

fn extract_user_id(headers: &axum::http::HeaderMap, jwt_secret: &str) -> Option<Uuid> {
    let auth = headers.get("Authorization")?.to_str().ok()?;
    let token = auth.strip_prefix("Bearer ")?;
    let key = jsonwebtoken::DecodingKey::from_secret(jwt_secret.as_bytes());
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.validate_exp = false;
    let data = jsonwebtoken::decode::<serde_json::Value>(token, &key, &validation).ok()?;
    let uid = data.claims.get("sub")?.as_str()?;
    uid.parse::<Uuid>().ok()
}

// ─── Error mapping ────────────────────────────────────────────────────────────

fn ticket_err_response(err: TicketError) -> (StatusCode, Json<Value>) {
    match err {
        TicketError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(
                json!({ "success": false, "message": "Ticket not found", "error_code": "NOT_FOUND" }),
            ),
        ),
        TicketError::Forbidden => (
            StatusCode::FORBIDDEN,
            Json(
                json!({ "success": false, "message": "Access denied", "error_code": "FORBIDDEN" }),
            ),
        ),
        TicketError::InvalidPriority(msg) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "message": msg, "error_code": "INVALID_PRIORITY" })),
        ),
        TicketError::InvalidStatus(msg) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "message": msg, "error_code": "INVALID_STATUS" })),
        ),
        TicketError::InvalidTransition(msg) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "success": false, "message": msg, "error_code": "INVALID_TRANSITION" })),
        ),
        TicketError::TicketClosed => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                json!({ "success": false, "message": "Ticket is CLOSED — no further changes allowed", "error_code": "TICKET_CLOSED" }),
            ),
        ),
        TicketError::MissingFields(msg) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "message": msg, "error_code": "MISSING_FIELDS" })),
        ),
        TicketError::DbError(e) => {
            tracing::error!("Ticket DB error: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    json!({ "success": false, "message": "Internal server error", "error_code": "DB_ERROR" }),
                ),
            )
        }
    }
}

// ─── 1. POST /api/v1/carecrew/tickets ─────────────────────────────────────────

pub async fn create_ticket_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<CreateTicketRequest>,
) -> (StatusCode, Json<Value>) {
    let user_id = match extract_user_id(&headers, &state.jwt_secret) {
        Some(u) => u,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(
                    json!({ "success": false, "message": "Unauthorized", "error_code": "UNAUTHORIZED" }),
                ),
            );
        }
    };

    match svc::create_ticket(
        &state.db,
        user_id,
        body.property_id,
        body.issue_type,
        body.description,
        body.priority,
    )
    .await
    {
        Ok(ticket) => (
            StatusCode::CREATED,
            Json(
                json!({ "success": true, "message": "Ticket created successfully", "data": { "ticket": ticket } }),
            ),
        ),
        Err(e) => ticket_err_response(e),
    }
}

// ─── 2. GET /api/v1/carecrew/tickets ──────────────────────────────────────────

#[derive(Deserialize)]
pub struct ListTicketsQuery {
    pub status: Option<String>,
    pub priority: Option<String>,
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

pub async fn list_tickets_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Query(params): Query<ListTicketsQuery>,
) -> (StatusCode, Json<Value>) {
    let user_id = match extract_user_id(&headers, &state.jwt_secret) {
        Some(u) => u,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(
                    json!({ "success": false, "message": "Unauthorized", "error_code": "UNAUTHORIZED" }),
                ),
            );
        }
    };

    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(10).clamp(1, 50);

    match svc::list_tickets(
        &state.db,
        user_id,
        params.status,
        params.priority,
        page,
        limit,
    )
    .await
    {
        Ok((tickets, total)) => {
            let total_pages = ((total as f64) / (limit as f64)).ceil() as i64;
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": "Tickets retrieved successfully",
                    "data": {
                        "tickets": tickets,
                        "pagination": {
                            "total_count":   total,
                            "current_page":  page,
                            "total_pages":   total_pages,
                            "limit":         limit
                        }
                    }
                })),
            )
        }
        Err(e) => ticket_err_response(e),
    }
}

// ─── 3. GET /api/v1/carecrew/tickets/{ticketId} ───────────────────────────────

pub async fn get_ticket_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(ticket_id_str): Path<String>,
) -> (StatusCode, Json<Value>) {
    let user_id = match extract_user_id(&headers, &state.jwt_secret) {
        Some(u) => u,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(
                    json!({ "success": false, "message": "Unauthorized", "error_code": "UNAUTHORIZED" }),
                ),
            );
        }
    };

    let ticket_id = match ticket_id_str.parse::<Uuid>() {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(
                    json!({ "success": false, "message": "Invalid ticket ID", "error_code": "INVALID_UUID" }),
                ),
            );
        }
    };

    match svc::get_ticket_detail(&state.db, ticket_id, user_id).await {
        Ok(data) => (
            StatusCode::OK,
            Json(
                json!({ "success": true, "message": "Ticket retrieved successfully", "data": data }),
            ),
        ),
        Err(e) => ticket_err_response(e),
    }
}

// ─── 4. PATCH /api/v1/carecrew/tickets/{ticketId} ─────────────────────────────

pub async fn update_ticket_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(ticket_id_str): Path<String>,
    Json(body): Json<UpdateTicketRequest>,
) -> (StatusCode, Json<Value>) {
    let user_id = match extract_user_id(&headers, &state.jwt_secret) {
        Some(u) => u,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(
                    json!({ "success": false, "message": "Unauthorized", "error_code": "UNAUTHORIZED" }),
                ),
            );
        }
    };

    let ticket_id = match ticket_id_str.parse::<Uuid>() {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(
                    json!({ "success": false, "message": "Invalid ticket ID", "error_code": "INVALID_UUID" }),
                ),
            );
        }
    };

    match svc::update_ticket(&state.db, ticket_id, user_id, body.status, body.assignee_id).await {
        Ok(ticket) => (
            StatusCode::OK,
            Json(
                json!({ "success": true, "message": "Ticket updated successfully", "data": { "ticket": ticket } }),
            ),
        ),
        Err(e) => ticket_err_response(e),
    }
}

// ─── 5. POST /api/v1/carecrew/tickets/{ticketId}/comments ────────────────────

pub async fn add_comment_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(ticket_id_str): Path<String>,
    Json(body): Json<AddTicketCommentRequest>,
) -> (StatusCode, Json<Value>) {
    let user_id = match extract_user_id(&headers, &state.jwt_secret) {
        Some(u) => u,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(
                    json!({ "success": false, "message": "Unauthorized", "error_code": "UNAUTHORIZED" }),
                ),
            );
        }
    };

    let ticket_id = match ticket_id_str.parse::<Uuid>() {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(
                    json!({ "success": false, "message": "Invalid ticket ID", "error_code": "INVALID_UUID" }),
                ),
            );
        }
    };

    match svc::add_comment(&state.db, ticket_id, user_id, body.comment).await {
        Ok(comment) => (
            StatusCode::CREATED,
            Json(
                json!({ "success": true, "message": "Comment added successfully", "data": { "comment": comment } }),
            ),
        ),
        Err(e) => ticket_err_response(e),
    }
}

// ─── Unit Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::models::carecrew::{TicketPriority, TicketStatus, validate_ticket_transition};

    #[test]
    fn test_ticket_lifecycle_open_to_closed() {
        assert!(validate_ticket_transition("OPEN", "IN_PROGRESS").is_ok());
        assert!(validate_ticket_transition("IN_PROGRESS", "RESOLVED").is_ok());
        assert!(validate_ticket_transition("RESOLVED", "CLOSED").is_ok());
    }

    #[test]
    fn test_ticket_reopen_paths() {
        assert!(validate_ticket_transition("IN_PROGRESS", "OPEN").is_ok());
        assert!(validate_ticket_transition("RESOLVED", "IN_PROGRESS").is_ok());
    }

    #[test]
    fn test_cannot_skip_states() {
        assert!(validate_ticket_transition("OPEN", "RESOLVED").is_err());
        assert!(validate_ticket_transition("OPEN", "CLOSED").is_err());
    }

    #[test]
    fn test_closed_is_terminal() {
        assert!(validate_ticket_transition("CLOSED", "OPEN").is_err());
        assert!(validate_ticket_transition("CLOSED", "IN_PROGRESS").is_err());
        assert!(validate_ticket_transition("CLOSED", "RESOLVED").is_err());
        assert!(TicketStatus::Closed.is_terminal());
    }

    #[test]
    fn test_invalid_status_string() {
        assert!(validate_ticket_transition("PENDING", "OPEN").is_err());
        assert!(!TicketStatus::is_valid("PENDING"));
    }

    #[test]
    fn test_priority_validation() {
        assert!(TicketPriority::is_valid("LOW"));
        assert!(TicketPriority::is_valid("MEDIUM"));
        assert!(TicketPriority::is_valid("HIGH"));
        assert!(!TicketPriority::is_valid("CRITICAL"));
        assert!(!TicketPriority::is_valid(""));
    }

    #[test]
    fn test_priority_case_insensitive() {
        assert!(TicketPriority::is_valid("low"));
        assert!(TicketPriority::is_valid("High"));
        assert!(TicketPriority::is_valid("MEDIUM"));
    }
}
