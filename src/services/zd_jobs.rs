//! Zero Deposit (Credit Tokens) v2 Background Jobs & Analytics
//!
//! Includes:
//! 1. 15-minute token expiry job (expires past Day 30, closes docs_pending, handles locked balances)
//! 2. Co-applicant 7-day timeout auto-reject with code CO_APPLICANT_EXPIRED
//! 3. Expiry warning notifications (7 days & 2 days before expiry)
//! 4. Monthly report query for approval and rejection rates by city and income band

use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::services::{
    zd_notifications::notify_expiry_warning,
    zd_tier_job::run_nightly_tier_job,
    zd_token_service,
};

#[derive(Debug, Clone, Default)]
pub struct ExpiryJobSummary {
    pub expired_allocations: usize,
    pub closed_redemptions: usize,
    pub expired_applications: usize,
}

/// Runs every 15 minutes:
/// - Expires allocations past issue + 30 days (via `zd_tokens::expire`).
/// - Closes `docs_pending` redemptions as `expired`.
/// - Moves application to `expired` if nothing was released.
/// - Redemptions submitted before Day 30 keep locked tokens honored until the redemption closes.
pub async fn run_token_expiry_job(pool: &PgPool) -> Result<ExpiryJobSummary, sqlx::Error> {
    let mut summary = ExpiryJobSummary::default();

    // 1. Find all active allocations that have expired past day 30
    let expired_allocations = sqlx::query(
        r#"SELECT id, zero_deposit_id, available::FLOAT8 as available, locked::FLOAT8 as locked, redeemed::FLOAT8 as redeemed
           FROM zd_token_allocations
           WHERE status = 'active'::zd_token_allocation_status
             AND expires_at < NOW()"#
    )
    .fetch_all(pool)
    .await?;

    for row in expired_allocations {
        let alloc_id: Uuid = row.get("id");
        let app_id: Uuid = row.get("zero_deposit_id");
        let available: f64 = row.get("available");
        let locked: f64 = row.get("locked");
        let redeemed: f64 = row.get("redeemed");

        // Expire available tokens via token ledger
        if available > 0.0 {
            if let Ok(_) = zd_token_service::expire(pool, app_id, None, Some("Day 30 background expiry")).await {
                summary.expired_allocations += 1;
            }
        } else {
            // Mark allocation status as expired directly if available was already 0
            let _ = sqlx::query(
                "UPDATE zd_token_allocations SET status = 'expired'::zd_token_allocation_status, updated_at = NOW() WHERE id = $1"
            )
            .bind(alloc_id)
            .execute(pool)
            .await;
            summary.expired_allocations += 1;
        }

        // Close any docs_pending redemptions for this application as expired
        let closed_reds = sqlx::query(
            r#"UPDATE zd_redemptions
               SET status = 'expired'::zd_redemption_status, updated_at = NOW()
               WHERE zero_deposit_id = $1 AND status = 'docs_pending'::zd_redemption_status"#
        )
        .bind(app_id)
        .execute(pool)
        .await?;

        if closed_reds.rows_affected() > 0 {
            summary.closed_redemptions += closed_reds.rows_affected() as usize;
        }

        // If no tokens are locked and nothing was redeemed, transition application to 'expired'
        if locked <= 0.0 && redeemed <= 0.0 {
            let app_res = sqlx::query(
                r#"UPDATE zero_deposits
                   SET status = 'expired'::zero_deposit_status, updated_at = NOW()
                   WHERE id = $1 AND status IN ('tokens_issued'::zero_deposit_status, 'redemption_in_progress'::zero_deposit_status)"#
            )
            .bind(app_id)
            .execute(pool)
            .await?;

            if app_res.rows_affected() > 0 {
                summary.expired_applications += app_res.rows_affected() as usize;
            }
        }
    }

    Ok(summary)
}

/// Runs periodically: auto-rejects applications waiting for co-applicant past 7 days / due_by
pub async fn run_co_applicant_timeout_job(pool: &PgPool) -> Result<usize, sqlx::Error> {
    let expired_apps = sqlx::query(
        r#"SELECT z.id, z.user_id, ca.id as co_applicant_id
           FROM zero_deposits z
           JOIN zd_co_applicants ca ON ca.zero_deposit_id = z.id
           WHERE z.status = 'action_required'::zero_deposit_status
             AND ('add_co_applicant' = ANY(z.pending_actions) OR 'co_applicant' = ANY(z.pending_actions))
             AND ca.status = 'invited'
             AND (ca.invite_expires_at < NOW() OR ca.due_by < NOW())"#
    )
    .fetch_all(pool)
    .await?;

    let mut rejected_count = 0;
    for row in expired_apps {
        let app_id: Uuid = row.get("id");
        let user_id: Uuid = row.get("user_id");

        let mut tx = pool.begin().await?;

        // Update co-applicant status to expired
        let _ = sqlx::query("UPDATE zd_co_applicants SET status = 'expired', updated_at = NOW() WHERE zero_deposit_id = $1 AND status = 'invited'")
            .bind(app_id)
            .execute(&mut *tx)
            .await?;

        // Update zero_deposits to rejected with reason code CO_APPLICANT_EXPIRED
        let res = sqlx::query(
            r#"UPDATE zero_deposits
               SET status = 'rejected'::zero_deposit_status,
                   admin_remarks = 'Co-applicant invitation expired after 7 days',
                   decision_reasons = COALESCE(decision_reasons, '[]'::jsonb) || '["CO_APPLICANT_EXPIRED"]'::jsonb,
                   updated_at = NOW()
               WHERE id = $1 AND status = 'action_required'::zero_deposit_status"#
        )
        .bind(app_id)
        .execute(&mut *tx)
        .await?;

        if res.rows_affected() > 0 {
            rejected_count += 1;
        }

        // Add rejection notification
        let _ = sqlx::query(
            "INSERT INTO notifications (id, user_id, title, message, type, is_read, created_at)
             VALUES ($1, $2, 'Zero Deposit Application Expired', 'Your co-applicant invitation timed out after 7 days. Your application has been closed.', 'ZERO_DEPOSIT', false, NOW())",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .execute(&mut *tx)
        .await;

        tx.commit().await?;
    }

    Ok(rejected_count)
}

/// Sends notifications 7 days and 2 days before token expiry
pub async fn send_expiry_warning_job(pool: &PgPool) -> Result<usize, sqlx::Error> {
    // 1. Check for allocations expiring in approx 7 days (6.9 to 7.1 days)
    let seven_days = sqlx::query(
        r#"SELECT a.id, a.zero_deposit_id, a.expires_at, z.user_id
           FROM zd_token_allocations a
           JOIN zero_deposits z ON a.zero_deposit_id = z.id
           WHERE a.status = 'active'::zd_token_allocation_status
             AND a.available > 0
             AND a.expires_at BETWEEN NOW() + INTERVAL '6 days 20 hours' AND NOW() + INTERVAL '7 days'
             AND NOT EXISTS (
                 SELECT 1 FROM notifications n
                 WHERE n.user_id = z.user_id AND n.title LIKE '%7 Days%'
             )"#
    )
    .fetch_all(pool)
    .await?;

    let mut sent = 0;
    for r in seven_days {
        let user_id: Uuid = r.get("user_id");
        let expires_at: DateTime<Utc> = r.get("expires_at");
        notify_expiry_warning(pool, user_id, 7, expires_at).await;
        sent += 1;
    }

    // 2. Check for allocations expiring in approx 2 days (1.9 to 2.1 days)
    let two_days = sqlx::query(
        r#"SELECT a.id, a.zero_deposit_id, a.expires_at, z.user_id
           FROM zd_token_allocations a
           JOIN zero_deposits z ON a.zero_deposit_id = z.id
           WHERE a.status = 'active'::zd_token_allocation_status
             AND a.available > 0
             AND a.expires_at BETWEEN NOW() + INTERVAL '1 day 20 hours' AND NOW() + INTERVAL '2 days'
             AND NOT EXISTS (
                 SELECT 1 FROM notifications n
                 WHERE n.user_id = z.user_id AND n.title LIKE '%2 Days%'
             )"#
    )
    .fetch_all(pool)
    .await?;

    for r in two_days {
        let user_id: Uuid = r.get("user_id");
        let expires_at: DateTime<Utc> = r.get("expires_at");
        notify_expiry_warning(pool, user_id, 2, expires_at).await;
        sent += 1;
    }

    Ok(sent)
}

/// Generates monthly report for approval and rejection rates by city and income band
pub async fn get_monthly_approval_report(
    pool: &PgPool,
    year: i32,
    month: i32,
) -> Result<Value, sqlx::Error> {
    let rows = sqlx::query(
        r#"WITH dataset AS (
            SELECT
                COALESCE(p.city, 'Unknown') as city,
                CASE
                    WHEN COALESCE(z.monthly_income, 0) < 25000 THEN '< ₹25,000'
                    WHEN z.monthly_income < 50000 THEN '₹25,000 - ₹50,000'
                    WHEN z.monthly_income < 100000 THEN '₹50,000 - ₹1,00,000'
                    ELSE '> ₹1,00,000'
                END as income_band,
                z.status::TEXT as status
            FROM zero_deposits z
            LEFT JOIN properties p ON z.property_id = p.id
            WHERE EXTRACT(YEAR FROM z.created_at) = $1
              AND EXTRACT(MONTH FROM z.created_at) = $2
        )
        SELECT
            city,
            income_band,
            COUNT(*) as total_applied,
            COUNT(*) FILTER (WHERE status IN ('tokens_issued', 'redemption_in_progress', 'released')) as total_approved,
            COUNT(*) FILTER (WHERE status IN ('rejected', 'blocked')) as total_rejected,
            COUNT(*) FILTER (WHERE status IN ('applied', 'ai_review', 'action_required', 'ai_manual_review')) as total_pending
        FROM dataset
        GROUP BY city, income_band
        ORDER BY city, income_band"#
    )
    .bind(year as f64)
    .bind(month as f64)
    .fetch_all(pool)
    .await?;

    let mut report_rows = Vec::new();
    let mut grand_total = 0i64;
    let mut grand_approved = 0i64;
    let mut grand_rejected = 0i64;

    for row in rows {
        let city: String = row.get("city");
        let income_band: String = row.get("income_band");
        let total: i64 = row.get("total_applied");
        let approved: i64 = row.get("total_approved");
        let rejected: i64 = row.get("total_rejected");
        let pending: i64 = row.get("total_pending");

        let approval_rate = if total > 0 {
            (approved as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        grand_total += total;
        grand_approved += approved;
        grand_rejected += rejected;

        report_rows.push(json!({
            "city": city,
            "income_band": income_band,
            "total_applied": total,
            "total_approved": approved,
            "total_rejected": rejected,
            "total_pending": pending,
            "approval_rate_pct": (approval_rate * 100.0).round() / 100.0
        }));
    }

    let overall_approval_rate = if grand_total > 0 {
        (grand_approved as f64 / grand_total as f64) * 100.0
    } else {
        0.0
    };

    Ok(json!({
        "year": year,
        "month": month,
        "summary": {
            "total_applied": grand_total,
            "total_approved": grand_approved,
            "total_rejected": grand_rejected,
            "approval_rate_pct": (overall_approval_rate * 100.0).round() / 100.0
        },
        "breakdown": report_rows
    }))
}

/// Spawns the background job runner loop inside Tokio runtime
pub fn start_zd_background_workers(pool: PgPool) {
    tokio::spawn(async move {
        log::info!("Zero Deposit v2 background worker started.");

        let mut expiry_interval = tokio::time::interval(std::time::Duration::from_secs(900)); // 15 mins
        let mut timeout_interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // 1 hour
        let mut tier_interval = tokio::time::interval(std::time::Duration::from_secs(86400)); // 24 hours

        loop {
            tokio::select! {
                _ = expiry_interval.tick() => {
                    log::info!("Running 15-minute token expiry & notification check...");
                    if let Err(e) = run_token_expiry_job(&pool).await {
                        log::error!("Token expiry job error: {}", e);
                    }
                    if let Err(e) = send_expiry_warning_job(&pool).await {
                        log::error!("Expiry warning job error: {}", e);
                    }
                }
                _ = timeout_interval.tick() => {
                    log::info!("Running co-applicant timeout check...");
                    if let Err(e) = run_co_applicant_timeout_job(&pool).await {
                        log::error!("Co-applicant timeout job error: {}", e);
                    }
                }
                _ = tier_interval.tick() => {
                    log::info!("Running nightly tier recalculation job...");
                    if let Err(e) = run_nightly_tier_job(&pool).await {
                        log::error!("Nightly tier job error: {}", e);
                    }
                }
            }
        }
    });
}
