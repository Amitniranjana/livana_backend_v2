/// CareCrew Tickets Service
/// Business logic layer: row mapping, validation, state machine enforcement.

use serde_json::{json, Value};
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;

use crate::repository::carecrew_tickets_repository as repo;
use crate::models::carecrew::{validate_ticket_transition, TicketPriority, TicketStatus};

// ─── Domain Errors ────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum TicketError {
    NotFound,
    Forbidden,
    InvalidPriority(String),
    InvalidStatus(String),
    InvalidTransition(String),
    TicketClosed,
    MissingFields(String),
    DbError(sqlx::Error),
}

impl From<sqlx::Error> for TicketError {
    fn from(e: sqlx::Error) -> Self {
        TicketError::DbError(e)
    }
}

// ─── Row Mappers ─────────────────────────────────────────────────────────────

fn row_to_ticket_json(row: &sqlx::postgres::PgRow) -> Value {
    json!({
        "id":          row.try_get::<Uuid, _>("id").map(|u| u.to_string()).unwrap_or_default(),
        "userId":      row.try_get::<Uuid, _>("user_id").map(|u| u.to_string()).unwrap_or_default(),
        "propertyId":  row.try_get::<Option<Uuid>, _>("property_id").ok().flatten().map(|u| u.to_string()),
        "assigneeId":  row.try_get::<Option<Uuid>, _>("assignee_id").ok().flatten().map(|u| u.to_string()),
        "issueType":   row.try_get::<String, _>("issue_type").unwrap_or_default(),
        "description": row.try_get::<String, _>("description").unwrap_or_default(),
        "priority":    row.try_get::<String, _>("priority").unwrap_or_default(),
        "status":      row.try_get::<String, _>("status").unwrap_or_default(),
        "createdAt":   row.try_get::<chrono::DateTime<chrono::Utc>, _>("created_at").map(|t| t.to_rfc3339()).unwrap_or_default(),
        "updatedAt":   row.try_get::<chrono::DateTime<chrono::Utc>, _>("updated_at").map(|t| t.to_rfc3339()).unwrap_or_default(),
    })
}

fn row_to_comment_json(row: &sqlx::postgres::PgRow) -> Value {
    json!({
        "id":          row.try_get::<Uuid, _>("id").map(|u| u.to_string()).unwrap_or_default(),
        "ticketId":    row.try_get::<Uuid, _>("ticket_id").map(|u| u.to_string()).unwrap_or_default(),
        "commenterId": row.try_get::<Uuid, _>("commenter_id").map(|u| u.to_string()).unwrap_or_default(),
        "comment":     row.try_get::<String, _>("comment").unwrap_or_default(),
        "createdAt":   row.try_get::<chrono::DateTime<chrono::Utc>, _>("created_at").map(|t| t.to_rfc3339()).unwrap_or_default(),
    })
}

// ─── Service Functions ────────────────────────────────────────────────────────

/// Create a new ticket. Defaults to status=OPEN, priority=MEDIUM if not provided.
pub async fn create_ticket(
    db: &Pool<Postgres>,
    user_id: Uuid,
    property_id_str: Option<String>,
    issue_type: String,
    description: String,
    priority_str: Option<String>,
) -> Result<Value, TicketError> {
    if issue_type.trim().is_empty() {
        return Err(TicketError::MissingFields("issue_type is required".into()));
    }
    if description.trim().is_empty() {
        return Err(TicketError::MissingFields("description is required".into()));
    }

    // Validate and default priority
    let priority = priority_str.as_deref().unwrap_or("MEDIUM");
    if !TicketPriority::is_valid(priority) {
        return Err(TicketError::InvalidPriority(
            format!("'{}' is not valid. Use LOW, MEDIUM, or HIGH", priority)
        ));
    }
    let priority_upper = priority.to_uppercase();

    // Parse optional property_id
    let property_id: Option<Uuid> = match property_id_str {
        Some(ref s) => Some(s.parse::<Uuid>().map_err(|_| {
            TicketError::MissingFields("property_id must be a valid UUID".into())
        })?),
        None => None,
    };

    let ticket_id = Uuid::new_v4();
    let row = repo::create_ticket(
        db, ticket_id, user_id, property_id,
        &issue_type, &description, &priority_upper,
    ).await?;

    Ok(row_to_ticket_json(&row))
}

/// List paginated tickets for a user, with optional status/priority filtering.
pub async fn list_tickets(
    db: &Pool<Postgres>,
    user_id: Uuid,
    status_filter: Option<String>,
    priority_filter: Option<String>,
    page: i32,
    limit: i32,
) -> Result<(Vec<Value>, i64), TicketError> {
    let page  = page.max(1);
    let limit = limit.clamp(1, 50);

    // Validate filters if provided
    if let Some(ref s) = status_filter {
        if !TicketStatus::is_valid(s) {
            return Err(TicketError::InvalidStatus(
                format!("'{}' is not valid. Use OPEN, IN_PROGRESS, RESOLVED, or CLOSED", s)
            ));
        }
    }
    if let Some(ref p) = priority_filter {
        if !TicketPriority::is_valid(p) {
            return Err(TicketError::InvalidPriority(
                format!("'{}' is not valid. Use LOW, MEDIUM, or HIGH", p)
            ));
        }
    }

    let status_upper  = status_filter.as_deref().map(|s| s.to_uppercase());
    let priority_upper = priority_filter.as_deref().map(|p| p.to_uppercase());

    let rows = repo::list_tickets_for_user(
        db, user_id,
        status_upper.as_deref(), priority_upper.as_deref(),
        page, limit,
    ).await?;
    let total = repo::count_tickets_for_user(
        db, user_id,
        status_upper.as_deref(), priority_upper.as_deref(),
    ).await?;

    let tickets: Vec<Value> = rows.iter().map(row_to_ticket_json).collect();
    Ok((tickets, total))
}

/// Get a single ticket with all its comments. Returns Forbidden if user_id doesn't own it.
pub async fn get_ticket_detail(
    db: &Pool<Postgres>,
    ticket_id: Uuid,
    caller_user_id: Uuid,
) -> Result<Value, TicketError> {
    let row = repo::get_ticket_by_id(db, ticket_id).await?
        .ok_or(TicketError::NotFound)?;

    // Ownership check
    let owner_id: Uuid = row.try_get("user_id").map_err(|e| TicketError::DbError(e))?;
    if owner_id != caller_user_id {
        return Err(TicketError::Forbidden);
    }

    let ticket_json = row_to_ticket_json(&row);
    let comment_rows = repo::get_comments_for_ticket(db, ticket_id).await?;
    let comments: Vec<Value> = comment_rows.iter().map(row_to_comment_json).collect();

    Ok(json!({ "ticket": ticket_json, "comments": comments }))
}

/// Update ticket status and/or assign an agent. Enforces state machine.
pub async fn update_ticket(
    db: &Pool<Postgres>,
    ticket_id: Uuid,
    caller_user_id: Uuid,
    new_status_str: Option<String>,
    assignee_id_str: Option<String>,
) -> Result<Value, TicketError> {
    // Load current ticket
    let row = repo::get_ticket_by_id(db, ticket_id).await?
        .ok_or(TicketError::NotFound)?;

    let owner_id: Uuid = row.try_get("user_id").map_err(|e| TicketError::DbError(e))?;
    if owner_id != caller_user_id {
        return Err(TicketError::Forbidden);
    }

    let current_status: String = row.try_get("status").map_err(|e| TicketError::DbError(e))?;

    // Validate and enforce state machine if status is being updated
    let new_status_upper: Option<String> = if let Some(ref s) = new_status_str {
        if !TicketStatus::is_valid(s) {
            return Err(TicketError::InvalidStatus(
                format!("'{}' is not a valid status.", s)
            ));
        }
        let upper = s.to_uppercase();
        if current_status == "CLOSED" {
            return Err(TicketError::TicketClosed);
        }
        validate_ticket_transition(&current_status, &upper)
            .map_err(|e| TicketError::InvalidTransition(e))?;
        Some(upper)
    } else {
        None
    };

    // Parse optional assignee UUID
    let assignee_id: Option<Uuid> = match assignee_id_str {
        Some(ref s) => Some(s.parse::<Uuid>().map_err(|_| {
            TicketError::MissingFields("assignee_id must be a valid UUID".into())
        })?),
        None => None,
    };

    let updated = repo::update_ticket(
        db, ticket_id, new_status_upper.as_deref(), assignee_id,
    ).await?;

    Ok(row_to_ticket_json(&updated))
}

/// Add a comment to a ticket.
pub async fn add_comment(
    db: &Pool<Postgres>,
    ticket_id: Uuid,
    commenter_id: Uuid,
    comment: String,
) -> Result<Value, TicketError> {
    if comment.trim().is_empty() {
        return Err(TicketError::MissingFields("comment cannot be empty".into()));
    }

    // Verify ticket exists
    let row = repo::get_ticket_by_id(db, ticket_id).await?
        .ok_or(TicketError::NotFound)?;

    // Cannot comment on a closed ticket
    let status: String = row.try_get("status").map_err(|e| TicketError::DbError(e))?;
    if status == "CLOSED" {
        return Err(TicketError::TicketClosed);
    }

    let comment_id = Uuid::new_v4();
    let comment_row = repo::add_comment(db, comment_id, ticket_id, commenter_id, &comment).await?;
    Ok(row_to_comment_json(&comment_row))
}
