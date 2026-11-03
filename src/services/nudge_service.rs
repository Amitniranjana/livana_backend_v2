/// Issue 99 — Personalized reminder nudge optimization (v1: rule-based A/B)
///
/// Design:
///   - `NUDGE_EXPERIMENTS_ENABLED` env flag gates the whole feature.
///     When false/unset → existing behavior (fixed schedule) is unchanged.
///   - Variant assignment: deterministic hash of (tenant_id ‖ experiment_id)
///     so the same tenant always gets the same variant without a DB round-trip.
///     The assignment is also persisted for auditability.
///   - Every send is logged in `nudge_reminder_logs` (ML-ready schema).
///   - Actual SMS/call dispatch is logged as an info! stub — wire to AWS SNS
///     when the non-OTP SMS service is ready.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use sqlx::PgPool;
use uuid::Uuid;

use crate::dtos::nudge::{
    CreateExperimentRequest, ExperimentResponse, ExperimentResultsResponse,
    NudgeVariantConfig, TenantNudgeStrategy, VariantResult,
};

// ─── Feature flag ──────────────────────────────────────────────────────────

/// Returns true if nudge experiments are enabled via env var.
pub fn experiments_enabled() -> bool {
    std::env::var("NUDGE_EXPERIMENTS_ENABLED")
        .map(|v| v.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

// ─── Variant assignment ─────────────────────────────────────────────────────

/// Deterministically assign a tenant to a variant using a stable hash.
///
/// The same (tenant_id, experiment_id) pair always produces the same index,
/// so a tenant's variant never changes mid-experiment.
pub fn assign_variant_index(tenant_id: Uuid, experiment_id: Uuid, n_variants: usize) -> usize {
    if n_variants == 0 {
        return 0;
    }
    let mut hasher = DefaultHasher::new();
    tenant_id.hash(&mut hasher);
    experiment_id.hash(&mut hasher);
    (hasher.finish() as usize) % n_variants
}

/// Fetch the active experiment (first active one), resolve the tenant's variant,
/// persist the assignment, and return the `TenantNudgeStrategy`.
///
/// Falls back to a legacy strategy when no experiment is active or the feature
/// flag is off.
pub async fn resolve_nudge_strategy(
    db: &PgPool,
    tenant_id: Uuid,
) -> Result<TenantNudgeStrategy, String> {
    // Feature flag check — return legacy strategy immediately if disabled
    if !experiments_enabled() {
        return Ok(legacy_strategy(tenant_id));
    }

    // Find the first active experiment
    let exp_row = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT row_to_json(t) FROM (
            SELECT id, name, variants
            FROM nudge_experiments
            WHERE is_active = true
            ORDER BY created_at ASC
            LIMIT 1
        ) t
        "#,
    )
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?;

    let Some(exp) = exp_row else {
        // No active experiment → legacy path
        return Ok(legacy_strategy(tenant_id));
    };

    let experiment_id: Uuid = exp
        .get("id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| "Invalid experiment id in DB".to_string())?;

    let variants: Vec<NudgeVariantConfig> = exp
        .get("variants")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    if variants.is_empty() {
        return Ok(legacy_strategy(tenant_id));
    }

    // Check if assignment already persisted
    let existing_key: Option<String> = sqlx::query_scalar(
        "SELECT variant_key FROM nudge_variant_assignments WHERE tenant_id = $1 AND experiment_id = $2"
    )
    .bind(tenant_id)
    .bind(experiment_id)
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?;

    let variant_key = if let Some(key) = existing_key {
        key
    } else {
        // Assign and persist
        let idx = assign_variant_index(tenant_id, experiment_id, variants.len());
        let key = variants[idx].key.clone();

        sqlx::query(
            r#"
            INSERT INTO nudge_variant_assignments (tenant_id, experiment_id, variant_key)
            VALUES ($1, $2, $3)
            ON CONFLICT (tenant_id, experiment_id) DO NOTHING
            "#,
        )
        .bind(tenant_id)
        .bind(experiment_id)
        .bind(&key)
        .execute(db)
        .await
        .map_err(|e| e.to_string())?;

        key
    };

    // Resolve variant config
    let variant = variants
        .iter()
        .find(|v| v.key == variant_key)
        .cloned()
        .unwrap_or_else(|| variants[0].clone());

    Ok(TenantNudgeStrategy {
        tenant_id,
        experiment_id: Some(experiment_id),
        variant_key: Some(variant.key),
        channel: variant.channel,
        timing_offset_hours: variant.timing_offset_hours,
        is_legacy: false,
    })
}

/// Default strategy: push channel, zero timing offset (existing behavior).
fn legacy_strategy(tenant_id: Uuid) -> TenantNudgeStrategy {
    TenantNudgeStrategy {
        tenant_id,
        experiment_id: None,
        variant_key: None,
        channel: "push".to_string(),
        timing_offset_hours: 0,
        is_legacy: true,
    }
}

// ─── Send reminder ──────────────────────────────────────────────────────────

/// Send a reminder nudge to a tenant using the resolved strategy.
/// Logs the send event to `nudge_reminder_logs`.
///
/// Returns the log row ID.
pub async fn send_nudge_for_tenant(
    db: &PgPool,
    tenant_id: Uuid,
    zero_deposit_id: Option<Uuid>,
    message: Option<&str>,
) -> Result<Uuid, String> {
    let strategy = resolve_nudge_strategy(db, tenant_id).await?;

    // ── Dispatch (v1 stub) ────────────────────────────────────────────────
    // In production, dispatch to the appropriate channel:
    //   push  → existing notifications table (in-app push)
    //   sms   → AWS SNS send_sms (reuse the SMS client from auth/OTP)
    //   call  → IVR provider webhook
    //   email → SES
    // For v1 we log the intent and create the in-app notification.
    let channel = &strategy.channel;
    log::info!(
        "[nudge] tenant={} experiment={:?} variant={:?} channel={} offset_h={}",
        tenant_id,
        strategy.experiment_id,
        strategy.variant_key,
        channel,
        strategy.timing_offset_hours,
    );

    // For push channel: insert into notifications table (existing path)
    if channel == "push" {
        let msg = message.unwrap_or("Your repayment reminder is due. Please make your payment.");
        sqlx::query(
            r#"
            INSERT INTO notifications (user_id, title, message, type)
            VALUES ($1, 'Payment Reminder', $2, 'payment_reminder')
            "#,
        )
        .bind(tenant_id)
        .bind(msg)
        .execute(db)
        .await
        .map_err(|e| e.to_string())?;
    }
    // For sms/call/email: stub — actual dispatch wired when SNS non-OTP SMS
    // service is ready. The log row is still written so outcomes can be tracked.

    // ── Log the send event ───────────────────────────────────────────────────
    let log_id = Uuid::new_v4();
    let msg_preview = message
        .map(|m| m.chars().take(100).collect::<String>())
        .unwrap_or_else(|| "Payment Reminder".to_string());

    sqlx::query(
        r#"
        INSERT INTO nudge_reminder_logs
            (id, tenant_id, zero_deposit_id, experiment_id, variant_key,
             channel, message_preview, outcome)
        VALUES ($1, $2, $3, $4, $5, $6::nudge_channel, $7, 'pending')
        "#,
    )
    .bind(log_id)
    .bind(tenant_id)
    .bind(zero_deposit_id)
    .bind(strategy.experiment_id)
    .bind(&strategy.variant_key)
    .bind(channel.as_str())
    .bind(&msg_preview)
    .execute(db)
    .await
    .map_err(|e| e.to_string())?;

    Ok(log_id)
}

// ─── Create experiment ──────────────────────────────────────────────────────

pub async fn create_experiment(
    db: &PgPool,
    req: &CreateExperimentRequest,
) -> Result<ExperimentResponse, String> {
    if req.variants.is_empty() {
        return Err("At least one variant is required".to_string());
    }

    let variants_json =
        serde_json::to_value(&req.variants).map_err(|e| e.to_string())?;

    let row = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        INSERT INTO nudge_experiments (name, description, is_active, variants)
        VALUES ($1, $2, $3, $4)
        RETURNING row_to_json(nudge_experiments.*)
        "#,
    )
    .bind(&req.name)
    .bind(&req.description)
    .bind(req.is_active.unwrap_or(false))
    .bind(&variants_json)
    .fetch_one(db)
    .await
    .map_err(|e| e.to_string())?;

    parse_experiment_row(row)
}

fn parse_experiment_row(row: serde_json::Value) -> Result<ExperimentResponse, String> {
    let id = row
        .get("id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or("missing id")?;
    let name = row
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let description = row
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let is_active = row.get("is_active").and_then(|v| v.as_bool()).unwrap_or(false);
    let variants: Vec<NudgeVariantConfig> = row
        .get("variants")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    let created_at = row
        .get("created_at")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(chrono::Utc::now);
    let updated_at = row
        .get("updated_at")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(chrono::Utc::now);

    Ok(ExperimentResponse {
        id,
        name,
        description,
        is_active,
        variants,
        created_at,
        updated_at,
    })
}

// ─── Experiment results ──────────────────────────────────────────────────────

pub async fn get_experiment_results(
    db: &PgPool,
    experiment_id: Uuid,
) -> Result<ExperimentResultsResponse, String> {
    // Load experiment meta
    let exp_row = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT row_to_json(t) FROM (SELECT id, name, is_active, variants FROM nudge_experiments WHERE id = $1) t"
    )
    .bind(experiment_id)
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "Experiment not found".to_string())?;

    let experiment_name = exp_row
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let is_active = exp_row
        .get("is_active")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let variants_config: Vec<NudgeVariantConfig> = exp_row
        .get("variants")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    // Aggregate per variant from logs
    let agg_rows = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT row_to_json(t) FROM (
            SELECT
                variant_key,
                COUNT(*)                                                 AS sent_count,
                COUNT(*) FILTER (WHERE outcome = 'delivered')            AS delivered_count,
                COUNT(*) FILTER (WHERE outcome = 'opened')               AS opened_count,
                COUNT(*) FILTER (WHERE outcome = 'paid')                 AS paid_count,
                AVG(paid_within_hours) FILTER (WHERE outcome = 'paid')   AS avg_paid_within_hours
            FROM nudge_reminder_logs
            WHERE experiment_id = $1
            GROUP BY variant_key
        ) t
        "#,
    )
    .bind(experiment_id)
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())?;

    let mut total_sent: i64 = 0;

    let mut variant_results: Vec<VariantResult> = agg_rows
        .iter()
        .map(|row| {
            let variant_key = row
                .get("variant_key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let sent_count = row
                .get("sent_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let delivered_count = row
                .get("delivered_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let opened_count = row
                .get("opened_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let paid_count = row
                .get("paid_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let avg_paid_within_hours = row
                .get("avg_paid_within_hours")
                .and_then(|v| v.as_f64());

            let payment_conversion_rate = if sent_count > 0 {
                paid_count as f64 / sent_count as f64
            } else {
                0.0
            };

            total_sent += sent_count;

            // Resolve channel from variant config
            let channel = variants_config
                .iter()
                .find(|v| v.key == variant_key)
                .map(|v| v.channel.clone())
                .unwrap_or_else(|| "unknown".to_string());

            VariantResult {
                variant_key,
                channel,
                sent_count,
                delivered_count,
                opened_count,
                paid_count,
                payment_conversion_rate,
                avg_paid_within_hours,
            }
        })
        .collect();

    // Include variant keys from config even if no sends yet
    for vc in &variants_config {
        if !variant_results.iter().any(|r| r.variant_key == vc.key) {
            variant_results.push(VariantResult {
                variant_key: vc.key.clone(),
                channel: vc.channel.clone(),
                sent_count: 0,
                delivered_count: 0,
                opened_count: 0,
                paid_count: 0,
                payment_conversion_rate: 0.0,
                avg_paid_within_hours: None,
            });
        }
    }

    Ok(ExperimentResultsResponse {
        experiment_id,
        experiment_name,
        is_active,
        total_sent,
        variants: variant_results,
    })
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_assignment_is_deterministic() {
        let tenant = Uuid::new_v4();
        let exp = Uuid::new_v4();
        let idx1 = assign_variant_index(tenant, exp, 3);
        let idx2 = assign_variant_index(tenant, exp, 3);
        assert_eq!(idx1, idx2, "same inputs must always produce same index");
    }

    #[test]
    fn variant_assignment_distributes_across_variants() {
        let exp = Uuid::new_v4();
        let mut counts = [0usize; 3];
        for _ in 0..300 {
            let tenant = Uuid::new_v4();
            let idx = assign_variant_index(tenant, exp, 3);
            counts[idx] += 1;
        }
        // Each bucket should have roughly 100 ± 30 hits (basic sanity check)
        for &c in &counts {
            assert!(c > 50, "bucket too empty: {}", c);
            assert!(c < 200, "bucket too full: {}", c);
        }
    }

    #[test]
    fn variant_assignment_handles_single_variant() {
        let tenant = Uuid::new_v4();
        let exp = Uuid::new_v4();
        assert_eq!(assign_variant_index(tenant, exp, 1), 0);
    }

    #[test]
    fn legacy_strategy_has_push_channel() {
        let strategy = legacy_strategy(Uuid::new_v4());
        assert_eq!(strategy.channel, "push");
        assert!(strategy.is_legacy);
        assert!(strategy.experiment_id.is_none());
    }
}
