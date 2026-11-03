/// Issue 100 — Collections queue smart prioritization
/// DTOs for GET /api/admin/loans/defaults/prioritized

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Query params ──────────────────────────────────────────────────────────

/// Query parameters for the prioritized defaults endpoint.
/// Mirrors `ZeroDepositQuery` so callers can use the same filter conventions.
#[derive(Debug, Deserialize)]
pub struct PrioritizedDefaultsQuery {
    /// Optional: filter by status (almost always redundant since only defaulted
    /// rows are returned, but kept for parity with the Issue 67 list endpoint).
    pub status: Option<String>,
    /// Page size (default 50, max 200).
    pub limit: Option<i64>,
    /// Page offset.
    pub offset: Option<i64>,
}

// ─── Response item ─────────────────────────────────────────────────────────

/// One item in the prioritized defaults list.
///
/// Includes `score`, `days_overdue`, and `outstanding_amount` so the admin
/// can see exactly *why* a loan ranks where it does.
#[derive(Debug, Serialize)]
pub struct PrioritizedDefaultItem {
    // ── Identity ───────────────────────────────────────────────────────────
    pub zero_deposit_id: Uuid,
    pub user_id: Uuid,
    pub property_id: Option<Uuid>,

    // ── Priority score and its inputs ──────────────────────────────────────
    /// Computed priority score: higher → more urgent to collect first.
    pub score: f64,
    /// Number of days since the earliest unpaid due date (or since defaulted_at).
    pub days_overdue: f64,
    /// Sum of pending + overdue installments; falls back to requested_deposit_amount.
    pub outstanding_amount: f64,

    // ── Financial snapshot ─────────────────────────────────────────────────
    pub monthly_rent: f64,
    pub requested_deposit_amount: f64,
    pub monthly_income: f64,

    // ── Loan status ────────────────────────────────────────────────────────
    pub status: String,
    pub defaulted_at: Option<DateTime<Utc>>,

    // ── Timestamps ─────────────────────────────────────────────────────────
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
