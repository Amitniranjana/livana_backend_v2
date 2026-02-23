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
}

#[derive(Debug, Deserialize)]
pub struct UpdateBookingStatusRequest {
    /// Allowed values: pending | confirmed | in_progress | completed | cancelled
    pub status: String,
}

/// Valid booking status transitions:
///   pending → confirmed | cancelled
///   confirmed → in_progress | cancelled
///   in_progress → completed | cancelled
///   completed → (terminal, no further transitions)
///   cancelled → (terminal, no further transitions)
pub fn is_valid_status(s: &str) -> bool {
    matches!(s, "pending" | "confirmed" | "in_progress" | "completed" | "cancelled")
}

pub fn is_valid_transition(from: &str, to: &str) -> bool {
    match from {
        "pending"     => matches!(to, "confirmed" | "cancelled"),
        "confirmed"   => matches!(to, "in_progress" | "cancelled"),
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
