/// Issue 99 — Personalized reminder nudge optimization
/// DTOs for experiments + nudge sending + result reporting

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Experiment ────────────────────────────────────────────────────────────

/// One variant inside an experiment config.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NudgeVariantConfig {
    /// Unique key within the experiment, e.g. "control", "sms_early".
    pub key: String,
    /// Channel to use: "sms" | "push" | "call" | "email".
    pub channel: String,
    /// Hours to offset from the standard reminder time.
    /// Negative = earlier, positive = later.
    pub timing_offset_hours: i32,
    /// Optional human description.
    pub description: Option<String>,
}

/// Request body for creating a new experiment.
#[derive(Debug, Deserialize)]
pub struct CreateExperimentRequest {
    pub name: String,
    pub description: Option<String>,
    pub is_active: Option<bool>,
    pub variants: Vec<NudgeVariantConfig>,
}

/// Experiment row returned in API responses.
#[derive(Debug, Serialize)]
pub struct ExperimentResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub variants: Vec<NudgeVariantConfig>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ─── Variant assignment ────────────────────────────────────────────────────

/// The resolved channel + timing for a specific tenant, after assignment.
#[derive(Debug, Serialize, Clone)]
pub struct TenantNudgeStrategy {
    pub tenant_id: Uuid,
    pub experiment_id: Option<Uuid>,
    pub variant_key: Option<String>,
    pub channel: String,
    pub timing_offset_hours: i32,
    /// True if the legacy (no-experiment) path was used.
    pub is_legacy: bool,
}

// ─── Send nudge ────────────────────────────────────────────────────────────

/// Request body for POST /api/admin/notifications/nudge
#[derive(Debug, Deserialize)]
pub struct SendNudgeRequest {
    /// The tenant (user) to nudge.
    pub tenant_id: Uuid,
    /// Optionally pin to a specific zero-deposit record for context.
    pub zero_deposit_id: Option<Uuid>,
    /// Message content (for push/SMS). Required if channel is sms or push.
    pub message: Option<String>,
}

// ─── Experiment results ────────────────────────────────────────────────────

/// Per-variant outcome summary returned by GET /experiments/:id/results
#[derive(Debug, Serialize)]
pub struct VariantResult {
    pub variant_key: String,
    pub channel: String,
    pub sent_count: i64,
    pub delivered_count: i64,
    pub opened_count: i64,
    pub paid_count: i64,
    /// paid_count / sent_count, 0.0 if sent_count == 0.
    pub payment_conversion_rate: f64,
    pub avg_paid_within_hours: Option<f64>,
}

/// Full results response for an experiment.
#[derive(Debug, Serialize)]
pub struct ExperimentResultsResponse {
    pub experiment_id: Uuid,
    pub experiment_name: String,
    pub is_active: bool,
    pub total_sent: i64,
    pub variants: Vec<VariantResult>,
}
