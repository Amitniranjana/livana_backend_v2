/// CareCrew Models
/// Rust structs for CareCrew services, providers, and bookings.
use serde::{Deserialize, Serialize};

// ─── Service ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct CareCrewService {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub category: Option<String>,
    pub is_active: bool,
}

// ─── Provider ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct CareCrewProvider {
    pub id: String,
    pub name: String,
    pub bio: Option<String>,
    pub service_type: String,
    pub city: Option<String>,
    pub rating: f64,
    pub review_count: i32,
    pub is_featured: bool,
    pub avatar_url: Option<String>,
    pub phone: Option<String>,
}

// ─── Booking ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct CareCrewBooking {
    pub id: String,
    pub provider_id: String,
    pub service_id: String,
    pub user_id: String,
    pub scheduled_at: String,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: String,
}

// ─── Request DTOs ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateBookingRequest {
    pub provider_id: String,
    pub service_id: String,
    /// ISO 8601 string e.g. "2026-03-01T10:00:00Z"
    pub scheduled_at: String,
    pub notes: Option<String>,
    pub address: Option<String>,
    pub problem_description: Option<String>,
    pub contact_number: Option<String>,
    pub estimated_cost: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBookingStatusRequest {
    /// Allowed values: pending | confirmed | in_progress | completed | cancelled
    pub status: String,
    pub notes: Option<String>,
    pub estimated_cost: Option<f64>,
}

// ─── Response DTOs (API Endpoints 33, 34, 35, 36) ─────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserBookingResponse {
    pub booking_id: Uuid,
    pub booking_number: String,
    pub provider_id: Uuid,
    pub provider_name: String,
    pub provider_image: Option<String>,
    pub service_type: String,
    pub scheduled_date_time: String,
    pub status: String,
    pub address: Option<String>,
    pub estimated_cost: Option<f64>,
    pub created_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ProviderBookingResponse {
    pub booking_id: Uuid,
    pub booking_number: String,
    pub customer_name: String,
    pub customer_phone: Option<String>,
    pub customer_image: Option<String>,
    pub service_type: String,
    pub scheduled_date_time: String,
    pub status: String,
    pub address: Option<String>,
    pub problem_description: Option<String>,
    pub estimated_cost: Option<f64>,
    pub created_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TrackingStatusDto {
    pub status: String,
    pub timestamp: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BookingDetailsResponse {
    pub booking_id: Uuid,
    pub booking_number: String,
    pub provider_id: Uuid,
    pub provider_name: String,
    pub provider_phone: Option<String>,
    pub provider_image: Option<String>,
    pub provider_rating: f64,
    pub service_type: String,
    pub scheduled_date_time: String,
    pub status: String,
    pub address: Option<String>,
    pub problem_description: Option<String>,
    pub contact_number: Option<String>,
    pub estimated_cost: Option<f64>,
    pub final_cost: Option<f64>,
    pub payment_status: String,
    pub tracking_status: Vec<TrackingStatusDto>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

use uuid::Uuid;

/// Valid booking status transitions:
///   pending → confirmed | cancelled
///   confirmed → in_progress | cancelled
///   in_progress → completed | cancelled
///   completed → (terminal, no further transitions)
///   cancelled → (terminal, no further transitions)
pub fn is_valid_status(s: &str) -> bool {
    matches!(
        s,
        "pending" | "confirmed" | "in_progress" | "completed" | "cancelled"
    )
}

pub fn is_valid_transition(from: &str, to: &str) -> bool {
    match from {
        "pending" => matches!(to, "confirmed" | "cancelled"),
        "confirmed" => matches!(to, "in_progress" | "cancelled"),
        "in_progress" => matches!(to, "completed" | "cancelled"),
        _ => false, // completed and cancelled are terminal
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_statuses() {
        assert!(is_valid_status("pending"));
        assert!(is_valid_status("confirmed"));
        assert!(is_valid_status("in_progress"));
        assert!(is_valid_status("completed"));
        assert!(is_valid_status("cancelled"));
        assert!(!is_valid_status("active"));
        assert!(!is_valid_status("done"));
        assert!(!is_valid_status(""));
    }

    #[test]
    fn test_valid_transitions() {
        assert!(is_valid_transition("pending", "confirmed"));
        assert!(is_valid_transition("pending", "cancelled"));
        assert!(is_valid_transition("confirmed", "in_progress"));
        assert!(is_valid_transition("confirmed", "cancelled"));
        assert!(is_valid_transition("in_progress", "completed"));
        assert!(is_valid_transition("in_progress", "cancelled"));
    }

    #[test]
    fn test_invalid_transitions() {
        // Cannot go back
        assert!(!is_valid_transition("completed", "pending"));
        assert!(!is_valid_transition("cancelled", "pending"));
        assert!(!is_valid_transition("completed", "confirmed"));
        // Cannot skip stages
        assert!(!is_valid_transition("pending", "completed"));
        assert!(!is_valid_transition("pending", "in_progress"));
    }

    #[test]
    fn test_create_booking_request_has_required_fields() {
        let req = CreateBookingRequest {
            provider_id: "provider-uuid".to_string(),
            service_id: "service-uuid".to_string(),
            scheduled_at: "2026-03-01T10:00:00Z".to_string(),
            notes: Some("Please come early".to_string()),
        };
        assert!(!req.provider_id.is_empty());
        assert!(!req.service_id.is_empty());
        assert!(!req.scheduled_at.is_empty());
        assert!(req.notes.is_some());
    }

    #[test]
    fn test_update_status_request_deserialization() {
        let json_str = r#"{"status": "confirmed"}"#;
        let req: UpdateBookingStatusRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(req.status, "confirmed");
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// CareCrew Ticketing Models (Support Module)
// ═════════════════════════════════════════════════════════════════════════════

use std::fmt;
use std::str::FromStr;

// ─── Priority Enum ────────────────────────────────────────────────────────────

/// Ticket priority stored as VARCHAR in Postgres.
/// DB values: 'LOW' | 'MEDIUM' | 'HIGH'
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TicketPriority {
    Low,
    Medium,
    High,
}

impl fmt::Display for TicketPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TicketPriority::Low => write!(f, "LOW"),
            TicketPriority::Medium => write!(f, "MEDIUM"),
            TicketPriority::High => write!(f, "HIGH"),
        }
    }
}

impl FromStr for TicketPriority {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "LOW" => Ok(TicketPriority::Low),
            "MEDIUM" => Ok(TicketPriority::Medium),
            "HIGH" => Ok(TicketPriority::High),
            other => Err(format!("Unknown priority: '{}'", other)),
        }
    }
}

#[allow(dead_code)]
impl TicketPriority {
    pub fn is_valid(s: &str) -> bool {
        s.parse::<TicketPriority>().is_ok()
    }
}

// ─── Status Enum ──────────────────────────────────────────────────────────────

/// Ticket lifecycle status stored as VARCHAR in Postgres.
/// DB values: 'OPEN' | 'IN_PROGRESS' | 'RESOLVED' | 'CLOSED'
///
/// Valid transitions:
///   OPEN → IN_PROGRESS
///   IN_PROGRESS → RESOLVED | OPEN (re-open)
///   RESOLVED → CLOSED | IN_PROGRESS (re-open)
///   CLOSED → (terminal — no further transitions)
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TicketStatus {
    Open,
    InProgress,
    Resolved,
    Closed,
}

impl fmt::Display for TicketStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TicketStatus::Open => write!(f, "OPEN"),
            TicketStatus::InProgress => write!(f, "IN_PROGRESS"),
            TicketStatus::Resolved => write!(f, "RESOLVED"),
            TicketStatus::Closed => write!(f, "CLOSED"),
        }
    }
}

impl FromStr for TicketStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "OPEN" => Ok(TicketStatus::Open),
            "IN_PROGRESS" => Ok(TicketStatus::InProgress),
            "RESOLVED" => Ok(TicketStatus::Resolved),
            "CLOSED" => Ok(TicketStatus::Closed),
            other => Err(format!("Unknown status: '{}'", other)),
        }
    }
}

#[allow(dead_code)]
impl TicketStatus {
    pub fn is_valid(s: &str) -> bool {
        s.parse::<TicketStatus>().is_ok()
    }

    /// Returns true if this status is terminal (no further transitions allowed).
    pub fn is_terminal(&self) -> bool {
        matches!(self, TicketStatus::Closed)
    }

    /// Validates whether a state transition from `self` → `to` is allowed.
    pub fn can_transition_to(&self, to: &TicketStatus) -> bool {
        match self {
            TicketStatus::Open => matches!(to, TicketStatus::InProgress),
            TicketStatus::InProgress => matches!(to, TicketStatus::Resolved | TicketStatus::Open),
            TicketStatus::Resolved => matches!(to, TicketStatus::Closed | TicketStatus::InProgress),
            TicketStatus::Closed => false, // terminal
        }
    }
}

// ─── Ticket Struct ────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CareCrewTicket {
    pub id: String,
    pub user_id: String,
    pub property_id: Option<String>,
    pub assignee_id: Option<String>,
    pub issue_type: String,
    pub description: String,
    pub priority: String, // "LOW" | "MEDIUM" | "HIGH"
    pub status: String,   // "OPEN" | "IN_PROGRESS" | "RESOLVED" | "CLOSED"
    pub created_at: String,
    pub updated_at: String,
}

// ─── Ticket Comment Struct ────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CareCrewTicketComment {
    pub id: String,
    pub ticket_id: String,
    pub commenter_id: String,
    pub comment: String,
    pub created_at: String,
}

// ─── Request DTOs ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateTicketRequest {
    pub property_id: Option<String>,
    /// e.g. "service", "operational", "billing", "other"
    pub issue_type: String,
    pub description: String,
    /// "LOW" | "MEDIUM" | "HIGH" — defaults to "MEDIUM" if omitted
    pub priority: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTicketRequest {
    /// New status — must follow state machine transitions
    pub status: Option<String>,
    /// UUID of the agent to assign
    pub assignee_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddTicketCommentRequest {
    pub comment: String,
}

// ─── Ticket State Machine Helpers ─────────────────────────────────────────────

/// Validate a raw status string and check if the transition from `from` → `to` is allowed.
/// Returns Err with a human-readable reason on failure.
#[allow(dead_code)]
pub fn validate_ticket_transition(from: &str, to: &str) -> Result<(), String> {
    let from_status = from
        .parse::<TicketStatus>()
        .map_err(|e| format!("Current status invalid: {}", e))?;
    let to_status = to
        .parse::<TicketStatus>()
        .map_err(|e| format!("Target status invalid: {}", e))?;

    if from_status.is_terminal() {
        return Err(format!(
            "Ticket is CLOSED — no further transitions are allowed"
        ));
    }
    if !from_status.can_transition_to(&to_status) {
        return Err(format!(
            "Cannot transition ticket from '{}' to '{}'. Valid next states: {}",
            from_status,
            to_status,
            valid_next_states(&from_status)
        ));
    }
    Ok(())
}

#[allow(dead_code)]
fn valid_next_states(status: &TicketStatus) -> &'static str {
    match status {
        TicketStatus::Open => "IN_PROGRESS",
        TicketStatus::InProgress => "RESOLVED, OPEN (re-open)",
        TicketStatus::Resolved => "CLOSED, IN_PROGRESS (re-open)",
        TicketStatus::Closed => "(none — terminal)",
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod ticket_tests {
    use super::*;

    // ── Priority ──────────────────────────────────────────────────────────────

    #[test]
    fn test_priority_from_str_valid() {
        assert_eq!(
            "LOW".parse::<TicketPriority>().unwrap(),
            TicketPriority::Low
        );
        assert_eq!(
            "MEDIUM".parse::<TicketPriority>().unwrap(),
            TicketPriority::Medium
        );
        assert_eq!(
            "HIGH".parse::<TicketPriority>().unwrap(),
            TicketPriority::High
        );
    }

    #[test]
    fn test_priority_from_str_case_insensitive() {
        assert_eq!(
            "low".parse::<TicketPriority>().unwrap(),
            TicketPriority::Low
        );
        assert_eq!(
            "High".parse::<TicketPriority>().unwrap(),
            TicketPriority::High
        );
    }

    #[test]
    fn test_priority_from_str_invalid() {
        assert!("CRITICAL".parse::<TicketPriority>().is_err());
        assert!("".parse::<TicketPriority>().is_err());
    }

    #[test]
    fn test_priority_display() {
        assert_eq!(TicketPriority::Low.to_string(), "LOW");
        assert_eq!(TicketPriority::Medium.to_string(), "MEDIUM");
        assert_eq!(TicketPriority::High.to_string(), "HIGH");
    }

    #[test]
    fn test_priority_is_valid() {
        assert!(TicketPriority::is_valid("LOW"));
        assert!(TicketPriority::is_valid("medium"));
        assert!(!TicketPriority::is_valid("URGENT"));
    }

    // ── Status ────────────────────────────────────────────────────────────────

    #[test]
    fn test_status_from_str_valid() {
        assert_eq!("OPEN".parse::<TicketStatus>().unwrap(), TicketStatus::Open);
        assert_eq!(
            "IN_PROGRESS".parse::<TicketStatus>().unwrap(),
            TicketStatus::InProgress
        );
        assert_eq!(
            "RESOLVED".parse::<TicketStatus>().unwrap(),
            TicketStatus::Resolved
        );
        assert_eq!(
            "CLOSED".parse::<TicketStatus>().unwrap(),
            TicketStatus::Closed
        );
    }

    #[test]
    fn test_status_from_str_invalid() {
        assert!("PENDING".parse::<TicketStatus>().is_err());
        assert!("".parse::<TicketStatus>().is_err());
        assert!("closed_forever".parse::<TicketStatus>().is_err());
    }

    #[test]
    fn test_status_display() {
        assert_eq!(TicketStatus::Open.to_string(), "OPEN");
        assert_eq!(TicketStatus::InProgress.to_string(), "IN_PROGRESS");
        assert_eq!(TicketStatus::Resolved.to_string(), "RESOLVED");
        assert_eq!(TicketStatus::Closed.to_string(), "CLOSED");
    }

    #[test]
    fn test_closed_is_terminal() {
        assert!(TicketStatus::Closed.is_terminal());
        assert!(!TicketStatus::Open.is_terminal());
        assert!(!TicketStatus::InProgress.is_terminal());
        assert!(!TicketStatus::Resolved.is_terminal());
    }

    // ── State Machine Transitions ─────────────────────────────────────────────

    #[test]
    fn test_valid_transitions() {
        // Normal forward flow
        assert!(TicketStatus::Open.can_transition_to(&TicketStatus::InProgress));
        assert!(TicketStatus::InProgress.can_transition_to(&TicketStatus::Resolved));
        assert!(TicketStatus::Resolved.can_transition_to(&TicketStatus::Closed));
        // Re-open paths
        assert!(TicketStatus::InProgress.can_transition_to(&TicketStatus::Open));
        assert!(TicketStatus::Resolved.can_transition_to(&TicketStatus::InProgress));
    }

    #[test]
    fn test_invalid_transitions() {
        // Cannot close directly from OPEN (must go through IN_PROGRESS first)
        assert!(!TicketStatus::Open.can_transition_to(&TicketStatus::Resolved));
        assert!(!TicketStatus::Open.can_transition_to(&TicketStatus::Closed));
        // Nothing from CLOSED
        assert!(!TicketStatus::Closed.can_transition_to(&TicketStatus::Open));
        assert!(!TicketStatus::Closed.can_transition_to(&TicketStatus::InProgress));
        assert!(!TicketStatus::Closed.can_transition_to(&TicketStatus::Resolved));
        // Cannot skip IN_PROGRESS → CLOSED directly from OPEN
        assert!(!TicketStatus::InProgress.can_transition_to(&TicketStatus::Closed));
    }

    #[test]
    fn test_validate_ticket_transition_ok() {
        assert!(validate_ticket_transition("OPEN", "IN_PROGRESS").is_ok());
        assert!(validate_ticket_transition("IN_PROGRESS", "RESOLVED").is_ok());
        assert!(validate_ticket_transition("RESOLVED", "CLOSED").is_ok());
        assert!(validate_ticket_transition("RESOLVED", "IN_PROGRESS").is_ok()); // re-open
    }

    #[test]
    fn test_validate_ticket_transition_closed_is_terminal() {
        let err = validate_ticket_transition("CLOSED", "OPEN").unwrap_err();
        assert!(err.contains("CLOSED"));
    }

    #[test]
    fn test_validate_ticket_transition_invalid_skip() {
        let err = validate_ticket_transition("OPEN", "CLOSED").unwrap_err();
        assert!(err.contains("Cannot transition"));
    }

    #[test]
    fn test_validate_ticket_transition_bad_status_string() {
        assert!(validate_ticket_transition("UNKNOWN", "OPEN").is_err());
        assert!(validate_ticket_transition("OPEN", "INVALID").is_err());
    }

    // ── Request DTOs ──────────────────────────────────────────────────────────

    #[test]
    fn test_create_ticket_request_deserialization() {
        let json = r#"{
            "issue_type": "service",
            "description": "Provider did not show up",
            "priority": "HIGH"
        }"#;
        let req: CreateTicketRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.issue_type, "service");
        assert_eq!(req.priority.as_deref(), Some("HIGH"));
        assert!(req.property_id.is_none());
    }

    #[test]
    fn test_create_ticket_request_optional_priority() {
        let json = r#"{"issue_type": "billing", "description": "Overcharged"}"#;
        let req: CreateTicketRequest = serde_json::from_str(json).unwrap();
        // priority defaults to MEDIUM when not provided
        assert!(req.priority.is_none());
    }

    #[test]
    fn test_update_ticket_request_deserialization() {
        let json =
            r#"{"status": "IN_PROGRESS", "assignee_id": "550e8400-e29b-41d4-a716-446655440000"}"#;
        let req: UpdateTicketRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.status.as_deref(), Some("IN_PROGRESS"));
        assert!(req.assignee_id.is_some());
    }

    #[test]
    fn test_add_comment_request_deserialization() {
        let json = r#"{"comment": "We have assigned an agent to your ticket."}"#;
        let req: AddTicketCommentRequest = serde_json::from_str(json).unwrap();
        assert!(!req.comment.is_empty());
    }
}
