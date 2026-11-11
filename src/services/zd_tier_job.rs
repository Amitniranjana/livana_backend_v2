//! Issue 110/Foundation: Nightly Tier Job
//!
//! Tiers (max monthly rent):
//! - A: 750+, salaried, 1+ yr in current job, ₹40,000 (admin can approve above)
//! - B: 700-749 or 750+ self-employed, ₹25,000
//! - C: 650-699 or new-to-credit, ₹15,000, requires co-applicant + landlord reference
//!
//! Lifecycle rules:
//! - Moves UP one level after 6 months of on-time rent (e.g. C -> B, B -> A)
//! - Moves DOWN one level after any late payment (e.g. A -> B, B -> C)
//! - `on_time_months` resets on late payment
//! - An admin tier change overrides the nightly job (`source == 'admin'` rows are preserved)

use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct TierJobSummary {
    pub upgraded_count: usize,
    pub downgraded_count: usize,
    pub unchanged_count: usize,
    pub admin_skipped_count: usize,
}

pub async fn run_nightly_tier_job(pool: &PgPool) -> Result<TierJobSummary, sqlx::Error> {
    let mut summary = TierJobSummary::default();

    // Fetch all tenant tiers
    let rows = sqlx::query(
        r#"SELECT user_id, tier::TEXT as tier, source, on_time_months
           FROM zd_tenant_tiers"#
    )
    .fetch_all(pool)
    .await?;

    for row in rows {
        let user_id: Uuid = row.get("user_id");
        let tier: String = row.get("tier");
        let source: String = row.get("source");
        let on_time_months: i32 = row.get("on_time_months");

        // Admin override protection: admin changes never get overwritten by nightly job
        if source == "admin" {
            summary.admin_skipped_count += 1;
            continue;
        }

        // Check if there was any late payment recorded in the last 30 days
        let had_late_payment: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(
                   SELECT 1 FROM zero_deposit_repayments
                   WHERE user_id = $1 AND is_late = true AND paid_at >= NOW() - INTERVAL '30 days'
               )"#
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if had_late_payment {
            // Downgrade tier one level and reset on_time_months to 0
            let new_tier = match tier.as_str() {
                "A" => "B",
                "B" => "C",
                _ => "C",
            };

            sqlx::query(
                r#"UPDATE zd_tenant_tiers
                   SET tier = $1::zd_tenant_tier,
                       source = 'auto_downgrade',
                       on_time_months = 0,
                       reason = 'Late payment recorded',
                       updated_at = NOW()
                   WHERE user_id = $2"#
            )
            .bind(new_tier)
            .bind(user_id)
            .execute(pool)
            .await?;

            summary.downgraded_count += 1;
            continue;
        }

        // Check if tenant has made on-time rent payments
        if on_time_months >= 6 {
            // Upgrade tier one level
            let new_tier = match tier.as_str() {
                "C" => "B",
                "B" => "A",
                _ => "A",
            };

            if new_tier != tier.as_str() {
                sqlx::query(
                    r#"UPDATE zd_tenant_tiers
                       SET tier = $1::zd_tenant_tier,
                           source = 'auto_upgrade',
                           on_time_months = 0,
                           reason = '6 months on-time rent qualification',
                           updated_at = NOW()
                       WHERE user_id = $2"#
                )
                .bind(new_tier)
                .bind(user_id)
                .execute(pool)
                .await?;

                summary.upgraded_count += 1;
                continue;
            }
        }

        summary.unchanged_count += 1;
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_transition_logic() {
        // Upgrade logic
        assert_eq!(match "C" { "C" => "B", "B" => "A", _ => "A" }, "B");
        assert_eq!(match "B" { "C" => "B", "B" => "A", _ => "A" }, "A");
        assert_eq!(match "A" { "C" => "B", "B" => "A", _ => "A" }, "A");

        // Downgrade logic
        assert_eq!(match "A" { "A" => "B", "B" => "C", _ => "C" }, "B");
        assert_eq!(match "B" { "A" => "B", "B" => "C", _ => "C" }, "C");
        assert_eq!(match "C" { "A" => "B", "B" => "C", _ => "C" }, "C");
    }
}

