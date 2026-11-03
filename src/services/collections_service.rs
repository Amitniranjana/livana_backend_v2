/// Issue 100 — Collections queue smart prioritization
///
/// Architecture:
///   `DefaultScorer` trait — scoring algorithm is behind a trait so a
///   model-based scorer can be swapped in later without touching the handler.
///
///   `HeuristicScorer` — v1 implementation:
///       score = days_overdue × W_DAYS × outstanding_amount × W_AMOUNT
///
///   `ScoringWeights` — all weights live in one struct; no scattered magic numbers.
///
///   Query strategy: single SQL JOIN (no N+1) fetches all data; scoring happens
///   in a single Rust pass, then sorted DESC by score (ties: oldest due date first).

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::dtos::collections::{PrioritizedDefaultItem, PrioritizedDefaultsQuery};

// ─── Scoring weights ────────────────────────────────────────────────────────
// Grouped into one struct so they can be loaded from config/DB in a future
// iteration without changing the scorer interface.

/// Weights that tune the heuristic scorer.
/// Default: 1.0 for both — pure `days_overdue × outstanding_amount`.
pub struct ScoringWeights {
    /// Multiplier applied to `days_overdue`.
    pub weight_days: f64,
    /// Multiplier applied to `outstanding_amount`.
    pub weight_amount: f64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            weight_days: 1.0,
            weight_amount: 1.0,
        }
    }
}

// ─── Scorer trait ───────────────────────────────────────────────────────────

/// Inputs that any scorer implementation receives per defaulted loan.
#[derive(Debug, Clone)]
pub struct DefaultScoreInput {
    /// Days since the earliest unpaid due date (or since `defaulted_at`).
    pub days_overdue: f64,
    /// Sum of pending/overdue installments (or `requested_deposit_amount` fallback).
    pub outstanding_amount: f64,
}

/// Scoring algorithm contract.
///
/// Implement this trait to add a model-based scorer without touching the handler
/// or the fetch/sort logic.
pub trait DefaultScorer: Send + Sync {
    fn score(&self, input: &DefaultScoreInput) -> f64;
}

// ─── v1 implementation: heuristic scorer ────────────────────────────────────

/// Rule-based scorer: `score = days_overdue × W_DAYS × outstanding_amount × W_AMOUNT`.
pub struct HeuristicScorer {
    weights: ScoringWeights,
}

impl HeuristicScorer {
    pub fn new(weights: ScoringWeights) -> Self {
        Self { weights }
    }
}

impl Default for HeuristicScorer {
    fn default() -> Self {
        Self::new(ScoringWeights::default())
    }
}

impl DefaultScorer for HeuristicScorer {
    fn score(&self, input: &DefaultScoreInput) -> f64 {
        input.days_overdue * self.weights.weight_days
            * input.outstanding_amount * self.weights.weight_amount
    }
}

// ─── Internal row type (DB → scorer) ────────────────────────────────────────

struct DefaultRow {
    zero_deposit_id: Uuid,
    user_id: Uuid,
    property_id: Option<Uuid>,
    monthly_rent: f64,
    requested_deposit_amount: f64,
    monthly_income: f64,
    status: String,
    defaulted_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    days_overdue: f64,
    outstanding_amount: f64,
    earliest_due_date: Option<NaiveDate>,
}

// ─── Service function ────────────────────────────────────────────────────────

/// Fetch all defaulted loans, score them, and return sorted DESC by score.
///
/// A single SQL query computes `days_overdue` and `outstanding_amount` via a
/// LEFT JOIN on `repayment_schedules` — no N+1 queries.
pub async fn get_prioritized_defaults(
    db: &PgPool,
    query: &PrioritizedDefaultsQuery,
    scorer: &dyn DefaultScorer,
) -> Result<Vec<PrioritizedDefaultItem>, String> {
    let limit = query.limit.unwrap_or(50).min(200);
    let offset = query.offset.unwrap_or(0);

    // Fetch a large batch (up to limit) so we can score and re-sort in Rust.
    // For very large datasets (>10 k defaulted loans) this should be paginated
    // differently, but the table size makes this safe for now.
    let rows = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT row_to_json(t) FROM (
            SELECT
                zd.id                                                   AS zero_deposit_id,
                zd.user_id,
                zd.property_id,
                zd.monthly_rent::FLOAT8,
                zd.requested_deposit_amount::FLOAT8,
                zd.monthly_income::FLOAT8,
                zd.status::TEXT,
                zd.defaulted_at,
                zd.created_at,
                zd.updated_at,

                -- days_overdue: days from the earliest unpaid due date;
                -- fallback to days since defaulted_at (or updated_at) when no
                -- repayment schedule exists yet.
                COALESCE(
                    GREATEST(
                        EXTRACT(EPOCH FROM (NOW() - MIN(rs.due_date)::TIMESTAMPTZ))::FLOAT8 / 86400.0,
                        0.0
                    ),
                    GREATEST(
                        EXTRACT(EPOCH FROM (NOW() - COALESCE(zd.defaulted_at, zd.updated_at)))::FLOAT8 / 86400.0,
                        0.0
                    )
                )                                                       AS days_overdue,

                -- outstanding_amount: sum of pending/overdue installments;
                -- fallback to requested_deposit_amount when no schedule exists.
                COALESCE(
                    NULLIF(
                        SUM(CASE WHEN rs.status IN ('pending', 'overdue') THEN rs.amount_due ELSE 0 END),
                        0
                    ),
                    zd.requested_deposit_amount
                )::FLOAT8                                               AS outstanding_amount,

                -- tie-break column: oldest due date sorts first
                MIN(rs.due_date)                                        AS earliest_due_date

            FROM zero_deposits zd
            LEFT JOIN repayment_schedules rs
                   ON rs.zero_deposit_id = zd.id
            WHERE zd.is_defaulted = true
            GROUP BY
                zd.id, zd.user_id, zd.property_id,
                zd.monthly_rent, zd.requested_deposit_amount, zd.monthly_income,
                zd.status, zd.defaulted_at, zd.created_at, zd.updated_at
            LIMIT $1 OFFSET $2
        ) t
        "#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())?;

    // Parse rows
    let mut parsed: Vec<(f64, Option<NaiveDate>, PrioritizedDefaultItem)> = rows
        .into_iter()
        .filter_map(|row| parse_and_score_row(row, scorer))
        .collect();

    // Sort: primary DESC by score, tie-break ASC by earliest_due_date
    parsed.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.cmp(&b.1))
    });

    Ok(parsed.into_iter().map(|(_, _, item)| item).collect())
}

/// Parse one JSON row from the DB, compute the score, return (score, due_date, item).
fn parse_and_score_row(
    row: serde_json::Value,
    scorer: &dyn DefaultScorer,
) -> Option<(f64, Option<NaiveDate>, PrioritizedDefaultItem)> {
    let zero_deposit_id = row
        .get("zero_deposit_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())?;

    let user_id = row
        .get("user_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())?;

    let property_id = row
        .get("property_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());

    let monthly_rent = row
        .get("monthly_rent")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let requested_deposit_amount = row
        .get("requested_deposit_amount")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let monthly_income = row
        .get("monthly_income")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let status = row
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("blocked")
        .to_string();

    let defaulted_at = row
        .get("defaulted_at")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<DateTime<Utc>>().ok());

    let created_at = row
        .get("created_at")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<DateTime<Utc>>().ok())
        .unwrap_or_else(Utc::now);

    let updated_at = row
        .get("updated_at")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<DateTime<Utc>>().ok())
        .unwrap_or_else(Utc::now);

    let days_overdue = row
        .get("days_overdue")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
        .max(0.0);

    let outstanding_amount = row
        .get("outstanding_amount")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let earliest_due_date: Option<NaiveDate> = row
        .get("earliest_due_date")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok());

    let score_input = DefaultScoreInput {
        days_overdue,
        outstanding_amount,
    };
    let score = scorer.score(&score_input);

    let item = PrioritizedDefaultItem {
        zero_deposit_id,
        user_id,
        property_id,
        score,
        days_overdue,
        outstanding_amount,
        monthly_rent,
        requested_deposit_amount,
        monthly_income,
        status,
        defaulted_at,
        created_at,
        updated_at,
    };

    Some((score, earliest_due_date, item))
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heuristic_scorer_basic() {
        let scorer = HeuristicScorer::default();
        let input = DefaultScoreInput {
            days_overdue: 30.0,
            outstanding_amount: 15_000.0,
        };
        // score = 30 * 1.0 * 15000 * 1.0
        assert!((scorer.score(&input) - 450_000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn heuristic_scorer_with_custom_weights() {
        let scorer = HeuristicScorer::new(ScoringWeights {
            weight_days: 2.0,
            weight_amount: 0.5,
        });
        let input = DefaultScoreInput {
            days_overdue: 10.0,
            outstanding_amount: 20_000.0,
        };
        // score = 10 * 2.0 * 20000 * 0.5 = 200_000
        assert!((scorer.score(&input) - 200_000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn heuristic_scorer_zero_inputs() {
        let scorer = HeuristicScorer::default();
        let input = DefaultScoreInput {
            days_overdue: 0.0,
            outstanding_amount: 0.0,
        };
        assert_eq!(scorer.score(&input), 0.0);
    }

    #[test]
    fn sort_order_score_desc_then_due_date_asc() {
        // Two items with equal score; the one with older due date should rank first
        let d1 = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let d2 = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();

        let mut v: Vec<(f64, Option<NaiveDate>)> = vec![(1000.0, Some(d2)), (1000.0, Some(d1))];
        v.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.cmp(&b.1))
        });
        // d1 (Jan) should come first
        assert_eq!(v[0].1, Some(d1));
    }
}
