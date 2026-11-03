//! Issue 110 — Eligibility Rules Engine (Deterministic)
//!
//! Check order:
//!   1. KYC & fraud signals
//!   2. Credit bureau (CIBIL)
//!   3. Income & employment
//!   4. Bank statement signals
//!   5. Rental behaviour & co-applicant
//!   6. Tier assignment & token issuance decision
//!
//! Outputs one of: auto_approve | action_required | manual_review | auto_reject
//! The AI never rejects on its own — only manual_review codes trigger manual_review.
//! Thresholds are read from zd_ai_config.

use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EligibilityDecision {
    AutoApprove,
    ActionRequired,
    ManualReview,
    AutoReject,
}

/// Rule-side manual-review reason codes (never causes auto-reject).
pub const TIER_C_REVIEW: &str = "TIER_C_REVIEW";
pub const RENT_ABOVE_TIER_CAP: &str = "RENT_ABOVE_TIER_CAP";
pub const DPD_RECENT: &str = "DPD_RECENT";
pub const HIGH_ENQUIRIES: &str = "HIGH_ENQUIRIES";
pub const NTC_ALT_SCORING: &str = "NTC_ALT_SCORING";
pub const LANDLORD_REFERENCE_NEGATIVE: &str = "LANDLORD_REFERENCE_NEGATIVE";

/// Auto-reject reason codes.
pub const CIBIL_BELOW_MINIMUM: &str = "CIBIL_BELOW_MINIMUM";
pub const DPD_EXCEEDS_LIMIT: &str = "DPD_EXCEEDS_LIMIT";
pub const ACTIVE_WRITE_OFF: &str = "ACTIVE_WRITE_OFF";
pub const SUIT_FILED: &str = "SUIT_FILED";
pub const KYC_FRAUD_FLAG: &str = "KYC_FRAUD_FLAG";
pub const INCOME_MULTIPLE_INSUFFICIENT: &str = "INCOME_MULTIPLE_INSUFFICIENT";
pub const OBLIGATIONS_CAP_EXCEEDED: &str = "OBLIGATIONS_CAP_EXCEEDED";
pub const RENT_CAP_EXCEEDED: &str = "RENT_CAP_EXCEEDED";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSignal {
    pub code: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EligibilityResult {
    pub decision: EligibilityDecision,
    /// Tier assigned (None if auto_reject before tier can be computed)
    pub tier: Option<char>,
    pub reject_reasons: Vec<RuleSignal>,
    pub manual_review_reasons: Vec<RuleSignal>,
    pub action_required_reasons: Vec<RuleSignal>,
    pub rules_version: String,
}

/// Input snapshot passed to the engine.
#[derive(Debug, Clone)]
pub struct ApplicationSignals {
    pub application_id: Uuid,
    pub monthly_income: f64,
    pub monthly_rent: f64,
    pub deposit_amount: f64,
    pub employment_type: String, // "salaried" | "self_employed" | "business_owner"
    // Bureau
    pub cibil_score: Option<i32>,
    pub dpd_12m: Option<i32>,
    pub dpd_24m: Option<i32>,
    pub enquiries_3m: Option<i32>,
    pub emi_total: Option<f64>,
    pub card_dues: Option<f64>,
    pub write_offs: Option<i32>,
    pub settlements: Option<i32>,
    pub suit_filed: bool,
    // Bank statement
    pub bounced_cheque_count: Option<i32>,
    pub avg_monthly_credit: Option<f64>,
    pub high_debt_load_flag: bool,
    // KYC / fraud
    pub kyc_fraud_flag: bool,
    // Rental behaviour
    pub is_repeat_renter: bool,
    pub landlord_reference_status: Option<String>, // "positive" | "negative" | "unreachable"
    // Co-applicant (if present and within 7-day window)
    pub co_applicant_income: Option<f64>,
    pub co_applicant_cibil: Option<i32>,
    // AI flags
    pub ai_flags: Vec<String>,
}

#[derive(Debug, Clone)]
struct Config {
    salaried_income_multiple: f64,
    self_employed_income_multiple: f64,
    obligations_cap_pct: f64,
    rent_cap_pct: f64,
    tier_a_min_cibil: i32,
    tier_b_min_cibil: i32,
    tier_c_min_cibil: i32,
    tier_a_rent_cap_pct: f64,
    tier_b_rent_cap_pct: f64,
    tier_c_rent_cap_pct: f64,
    max_dpd_12m: i32,
    max_dpd_24m: i32,
    max_enquiries_3m: i32,
    max_bounces_6m: i32,
    ntc_alt_scoring_enabled: bool,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the full deterministic eligibility check.
pub async fn run(pool: &PgPool, signals: &ApplicationSignals) -> Result<EligibilityResult, sqlx::Error> {
    let cfg = load_config(pool).await?;
    Ok(evaluate(&cfg, signals))
}

// ---------------------------------------------------------------------------
// Rules evaluation (pure, no I/O)
// ---------------------------------------------------------------------------

fn evaluate(cfg: &Config, s: &ApplicationSignals) -> EligibilityResult {
    let mut reject: Vec<RuleSignal> = Vec::new();
    let mut manual: Vec<RuleSignal> = Vec::new();
    let mut action: Vec<RuleSignal> = Vec::new();

    // ────────────────────────────────────────────────────────────────────────
    // Step 1 — KYC & Fraud
    // ────────────────────────────────────────────────────────────────────────
    if s.kyc_fraud_flag {
        reject.push(RuleSignal {
            code: KYC_FRAUD_FLAG.into(),
            detail: "KYC or fraud flag raised for this applicant".into(),
        });
    }

    // ────────────────────────────────────────────────────────────────────────
    // Step 2 — Bureau checks
    // ────────────────────────────────────────────────────────────────────────
    if s.suit_filed {
        reject.push(RuleSignal {
            code: SUIT_FILED.into(),
            detail: "Suit filed on credit profile".into(),
        });
    }

    if let Some(wo) = s.write_offs {
        if wo > 0 {
            reject.push(RuleSignal {
                code: ACTIVE_WRITE_OFF.into(),
                detail: format!("{} write-off(s) found on bureau", wo),
            });
        }
    }

    let is_ntc = s.cibil_score.is_none();

    if let Some(score) = s.cibil_score {
        if score < cfg.tier_c_min_cibil {
            reject.push(RuleSignal {
                code: CIBIL_BELOW_MINIMUM.into(),
                detail: format!(
                    "CIBIL score {} is below minimum threshold {}",
                    score, cfg.tier_c_min_cibil
                ),
            });
        }
    }

    if let Some(dpd12) = s.dpd_12m {
        if dpd12 > cfg.max_dpd_12m {
            // DPD recent → manual review, not auto-reject
            manual.push(RuleSignal {
                code: DPD_RECENT.into(),
                detail: format!(
                    "DPD in last 12 months: {} (threshold: {})",
                    dpd12, cfg.max_dpd_12m
                ),
            });
        }
    }

    if let Some(dpd24) = s.dpd_24m {
        if dpd24 > cfg.max_dpd_24m {
            reject.push(RuleSignal {
                code: DPD_EXCEEDS_LIMIT.into(),
                detail: format!(
                    "DPD in last 24 months: {} (threshold: {})",
                    dpd24, cfg.max_dpd_24m
                ),
            });
        }
    }

    if let Some(enq) = s.enquiries_3m {
        if enq > cfg.max_enquiries_3m {
            manual.push(RuleSignal {
                code: HIGH_ENQUIRIES.into(),
                detail: format!(
                    "Enquiries in last 3 months: {} (threshold: {})",
                    enq, cfg.max_enquiries_3m
                ),
            });
        }
    }

    // ────────────────────────────────────────────────────────────────────────
    // Step 3 — Income & Employment
    // ────────────────────────────────────────────────────────────────────────
    let income_multiple = match s.employment_type.as_str() {
        "self_employed" => cfg.self_employed_income_multiple,
        "business_owner" => cfg.self_employed_income_multiple, // same as self-employed
        _ => cfg.salaried_income_multiple, // salaried default
    };

    // Effective income = own + co-applicant (if applicable)
    let effective_income = s.monthly_income
        + s.co_applicant_income.unwrap_or(0.0);

    if effective_income * income_multiple < s.deposit_amount {
        // Check if income is just insufficient
        if s.monthly_income * income_multiple < s.deposit_amount {
            if s.co_applicant_income.is_some() {
                // Co-applicant didn't help enough either
                reject.push(RuleSignal {
                    code: INCOME_MULTIPLE_INSUFFICIENT.into(),
                    detail: format!(
                        "Combined income {:.0} × {:.1} = {:.0} < deposit {:.0}",
                        effective_income,
                        income_multiple,
                        effective_income * income_multiple,
                        s.deposit_amount
                    ),
                });
            } else {
                // Co-applicant might help — mark action_required
                action.push(RuleSignal {
                    code: "CO_APPLICANT_NEEDED".into(),
                    detail: format!(
                        "Income {:.0} × {:.1} = {:.0} < deposit {:.0}. Co-applicant may qualify.",
                        s.monthly_income,
                        income_multiple,
                        s.monthly_income * income_multiple,
                        s.deposit_amount
                    ),
                });
            }
        }
    }

    // 50% obligations cap: (existing_emi + rent) / income <= 50%
    let existing_obligations = s.emi_total.unwrap_or(0.0) + s.card_dues.unwrap_or(0.0) * 0.05; // 5% of card dues as monthly obligation
    let total_obligations = existing_obligations + s.monthly_rent;
    if effective_income > 0.0
        && (total_obligations / effective_income) * 100.0 > cfg.obligations_cap_pct
    {
        reject.push(RuleSignal {
            code: OBLIGATIONS_CAP_EXCEEDED.into(),
            detail: format!(
                "Total obligations {:.0} / income {:.0} = {:.1}% > {:.0}% cap",
                total_obligations,
                effective_income,
                (total_obligations / effective_income) * 100.0,
                cfg.obligations_cap_pct
            ),
        });
    }

    // 35% rent cap: rent / income <= 35%
    if effective_income > 0.0
        && (s.monthly_rent / effective_income) * 100.0 > cfg.rent_cap_pct
    {
        reject.push(RuleSignal {
            code: RENT_CAP_EXCEEDED.into(),
            detail: format!(
                "Rent {:.0} / income {:.0} = {:.1}% > {:.0}% cap",
                s.monthly_rent,
                effective_income,
                (s.monthly_rent / effective_income) * 100.0,
                cfg.rent_cap_pct
            ),
        });
    }

    // NTC (No Credit Track) alt-scoring path
    if is_ntc {
        if cfg.ntc_alt_scoring_enabled {
            manual.push(RuleSignal {
                code: NTC_ALT_SCORING.into(),
                detail: "No credit track — alternative scoring required".into(),
            });
        } else {
            reject.push(RuleSignal {
                code: CIBIL_BELOW_MINIMUM.into(),
                detail: "No credit track and NTC alternative scoring is disabled".into(),
            });
        }
    }

    // ────────────────────────────────────────────────────────────────────────
    // Step 4 — Bank statement signals
    // ────────────────────────────────────────────────────────────────────────
    if let Some(bounces) = s.bounced_cheque_count {
        if bounces > cfg.max_bounces_6m {
            manual.push(RuleSignal {
                code: "HIGH_BOUNCES".into(),
                detail: format!(
                    "{} bounced cheques in 6 months (threshold: {})",
                    bounces, cfg.max_bounces_6m
                ),
            });
        }
    }

    if s.high_debt_load_flag {
        manual.push(RuleSignal {
            code: "HIGH_DEBT_LOAD".into(),
            detail: "High debt load detected in bank statement".into(),
        });
    }

    // ────────────────────────────────────────────────────────────────────────
    // Step 5 — Rental behaviour & co-applicant
    // ────────────────────────────────────────────────────────────────────────
    if let Some(ref landlord_status) = s.landlord_reference_status {
        if landlord_status == "negative" {
            manual.push(RuleSignal {
                code: LANDLORD_REFERENCE_NEGATIVE.into(),
                detail: "Landlord reference check returned negative".into(),
            });
        }
    }

    // ────────────────────────────────────────────────────────────────────────
    // Step 6 — Tier assignment
    // ────────────────────────────────────────────────────────────────────────

    // Early exit if auto-reject triggered
    if !reject.is_empty() {
        return EligibilityResult {
            decision: EligibilityDecision::AutoReject,
            tier: None,
            reject_reasons: reject,
            manual_review_reasons: manual,
            action_required_reasons: action,
            rules_version: "v2.0".into(),
        };
    }

    // Assign tier based on CIBIL
    let tier = assign_tier(cfg, s);

    // Tier-specific rent cap check
    let tier_rent_cap = match tier {
        'A' => cfg.tier_a_rent_cap_pct,
        'B' => cfg.tier_b_rent_cap_pct,
        _ => cfg.tier_c_rent_cap_pct,
    };

    if effective_income > 0.0
        && (s.monthly_rent / effective_income) * 100.0 > tier_rent_cap
    {
        manual.push(RuleSignal {
            code: RENT_ABOVE_TIER_CAP.into(),
            detail: format!(
                "Rent {:.0} / income {:.0} = {:.1}% > Tier {} cap {:.0}%",
                s.monthly_rent,
                effective_income,
                (s.monthly_rent / effective_income) * 100.0,
                tier,
                tier_rent_cap
            ),
        });
    }

    // Tier C → manual review
    if tier == 'C' {
        manual.push(RuleSignal {
            code: TIER_C_REVIEW.into(),
            detail: "Tier C applicants require manual review".into(),
        });
    }

    // ────────────────────────────────────────────────────────────────────────
    // Final decision
    // ────────────────────────────────────────────────────────────────────────
    let decision = if !action.is_empty() {
        EligibilityDecision::ActionRequired
    } else if !manual.is_empty() {
        EligibilityDecision::ManualReview
    } else {
        EligibilityDecision::AutoApprove
    };

    EligibilityResult {
        decision,
        tier: Some(tier),
        reject_reasons: reject,
        manual_review_reasons: manual,
        action_required_reasons: action,
        rules_version: "v2.0".into(),
    }
}

fn assign_tier(cfg: &Config, s: &ApplicationSignals) -> char {
    match s.cibil_score {
        Some(score) if score >= cfg.tier_a_min_cibil => 'A',
        Some(score) if score >= cfg.tier_b_min_cibil => 'B',
        Some(score) if score >= cfg.tier_c_min_cibil => 'C',
        None => 'C', // NTC → Tier C (subject to alt scoring)
        _ => 'C',   // Below minimum — shouldn't reach here (already rejected above)
    }
}

// ---------------------------------------------------------------------------
// Config loader
// ---------------------------------------------------------------------------

async fn load_config(pool: &PgPool) -> Result<Config, sqlx::Error> {
    let row = sqlx::query(
        r#"SELECT
            salaried_income_multiple::FLOAT8,
            self_employed_income_multiple::FLOAT8,
            obligations_cap_pct::FLOAT8,
            rent_cap_pct::FLOAT8,
            tier_a_min_cibil,
            tier_b_min_cibil,
            tier_c_min_cibil,
            tier_a_rent_cap_pct::FLOAT8,
            tier_b_rent_cap_pct::FLOAT8,
            tier_c_rent_cap_pct::FLOAT8,
            max_dpd_12m,
            max_dpd_24m,
            max_enquiries_3m,
            max_bounces_6m,
            COALESCE(ntc_alt_scoring_enabled, TRUE) as ntc_alt_scoring_enabled
        FROM zd_ai_config
        LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await?;

    // If no config row, use safe defaults
    let row = match row {
        Some(r) => r,
        None => {
            return Ok(Config {
                salaried_income_multiple: 3.0,
                self_employed_income_multiple: 4.0,
                obligations_cap_pct: 50.0,
                rent_cap_pct: 35.0,
                tier_a_min_cibil: 750,
                tier_b_min_cibil: 650,
                tier_c_min_cibil: 600,
                tier_a_rent_cap_pct: 35.0,
                tier_b_rent_cap_pct: 30.0,
                tier_c_rent_cap_pct: 25.0,
                max_dpd_12m: 0,
                max_dpd_24m: 30,
                max_enquiries_3m: 5,
                max_bounces_6m: 2,
                ntc_alt_scoring_enabled: true,
            });
        }
    };

    Ok(Config {
        salaried_income_multiple: row.get("salaried_income_multiple"),
        self_employed_income_multiple: row.get("self_employed_income_multiple"),
        obligations_cap_pct: row.get("obligations_cap_pct"),
        rent_cap_pct: row.get("rent_cap_pct"),
        tier_a_min_cibil: row.get("tier_a_min_cibil"),
        tier_b_min_cibil: row.get("tier_b_min_cibil"),
        tier_c_min_cibil: row.get("tier_c_min_cibil"),
        tier_a_rent_cap_pct: row.get("tier_a_rent_cap_pct"),
        tier_b_rent_cap_pct: row.get("tier_b_rent_cap_pct"),
        tier_c_rent_cap_pct: row.get("tier_c_rent_cap_pct"),
        max_dpd_12m: row.get("max_dpd_12m"),
        max_dpd_24m: row.get("max_dpd_24m"),
        max_enquiries_3m: row.get("max_enquiries_3m"),
        max_bounces_6m: row.get("max_bounces_6m"),
        ntc_alt_scoring_enabled: row.try_get("ntc_alt_scoring_enabled").unwrap_or(true),
    })
}

// ---------------------------------------------------------------------------
// Helper to persist the assessment result and tier
// ---------------------------------------------------------------------------

pub async fn persist_tier(
    pool: &PgPool,
    application_id: Uuid,
    tier: char,
    cibil_score: Option<i32>,
    income_multiple: f64,
    rent_cap_pct: f64,
    rules_version: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO zd_tenant_tiers
               (zero_deposit_id, tier, cibil_score, income_multiple, rent_cap_pct, rules_version)
           VALUES ($1, $2::zd_tenant_tier, $3, $4, $5, $6)
           ON CONFLICT (zero_deposit_id) DO UPDATE
           SET tier = EXCLUDED.tier,
               cibil_score = EXCLUDED.cibil_score,
               income_multiple = EXCLUDED.income_multiple,
               rent_cap_pct = EXCLUDED.rent_cap_pct,
               rules_version = EXCLUDED.rules_version,
               assigned_at = NOW()"#,
    )
    .bind(application_id)
    .bind(tier.to_string())
    .bind(cibil_score)
    .bind(income_multiple)
    .bind(rent_cap_pct)
    .bind(rules_version)
    .execute(pool)
    .await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Unit tests (Issue 110: Deterministic Rules Engine verification)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> Config {
        Config {
            salaried_income_multiple: 3.0,
            self_employed_income_multiple: 4.0,
            obligations_cap_pct: 50.0,
            rent_cap_pct: 35.0,
            tier_a_min_cibil: 750,
            tier_b_min_cibil: 650,
            tier_c_min_cibil: 600,
            tier_a_rent_cap_pct: 35.0,
            tier_b_rent_cap_pct: 30.0,
            tier_c_rent_cap_pct: 25.0,
            max_dpd_12m: 0,
            max_dpd_24m: 30,
            max_enquiries_3m: 5,
            max_bounces_6m: 2,
            ntc_alt_scoring_enabled: true,
        }
    }

    fn sample_applicant() -> ApplicationSignals {
        ApplicationSignals {
            application_id: Uuid::new_v4(),
            monthly_income: 100_000.0,
            monthly_rent: 25_000.0,
            deposit_amount: 50_000.0,
            employment_type: "salaried".into(),
            cibil_score: Some(780),
            dpd_12m: Some(0),
            dpd_24m: Some(0),
            enquiries_3m: Some(1),
            emi_total: Some(10_000.0),
            card_dues: Some(0.0),
            write_offs: Some(0),
            settlements: Some(0),
            suit_filed: false,
            bounced_cheque_count: Some(0),
            avg_monthly_credit: Some(100_000.0),
            high_debt_load_flag: false,
            kyc_fraud_flag: false,
            is_repeat_renter: false,
            landlord_reference_status: Some("positive".into()),
            co_applicant_income: None,
            co_applicant_cibil: None,
            ai_flags: vec![],
        }
    }

    #[test]
    fn test_tier_a_auto_approve() {
        let cfg = default_config();
        let app = sample_applicant();
        let res = evaluate(&cfg, &app);
        assert_eq!(res.decision, EligibilityDecision::AutoApprove);
        assert_eq!(res.tier, Some('A'));
        assert!(res.reject_reasons.is_empty());
        assert!(res.manual_review_reasons.is_empty());
    }

    #[test]
    fn test_tier_b_auto_approve() {
        let cfg = default_config();
        let mut app = sample_applicant();
        app.cibil_score = Some(680);
        let res = evaluate(&cfg, &app);
        assert_eq!(res.decision, EligibilityDecision::AutoApprove);
        assert_eq!(res.tier, Some('B'));
    }

    #[test]
    fn test_tier_c_triggers_manual_review() {
        let cfg = default_config();
        let mut app = sample_applicant();
        app.cibil_score = Some(620);
        app.monthly_rent = 20_000.0; // within 25% cap for tier C
        let res = evaluate(&cfg, &app);
        assert_eq!(res.decision, EligibilityDecision::ManualReview);
        assert_eq!(res.tier, Some('C'));
        assert!(res.manual_review_reasons.iter().any(|r| r.code == TIER_C_REVIEW));
    }

    #[test]
    fn test_cibil_below_600_auto_reject() {
        let cfg = default_config();
        let mut app = sample_applicant();
        app.cibil_score = Some(550);
        let res = evaluate(&cfg, &app);
        assert_eq!(res.decision, EligibilityDecision::AutoReject);
        assert!(res.reject_reasons.iter().any(|r| r.code == CIBIL_BELOW_MINIMUM));
    }

    #[test]
    fn test_obligations_cap_exceeded_auto_reject() {
        let cfg = default_config();
        let mut app = sample_applicant();
        // Rent 25k + EMI 30k = 55k / 100k = 55% > 50% cap
        app.emi_total = Some(30_000.0);
        let res = evaluate(&cfg, &app);
        assert_eq!(res.decision, EligibilityDecision::AutoReject);
        assert!(res.reject_reasons.iter().any(|r| r.code == OBLIGATIONS_CAP_EXCEEDED));
    }

    #[test]
    fn test_rent_cap_exceeded_auto_reject() {
        let cfg = default_config();
        let mut app = sample_applicant();
        // Rent 40k / 100k = 40% > 35% cap
        app.monthly_rent = 40_000.0;
        let res = evaluate(&cfg, &app);
        assert_eq!(res.decision, EligibilityDecision::AutoReject);
        assert!(res.reject_reasons.iter().any(|r| r.code == RENT_CAP_EXCEEDED));
    }

    #[test]
    fn test_dpd_recent_manual_review() {
        let cfg = default_config();
        let mut app = sample_applicant();
        app.dpd_12m = Some(15);
        let res = evaluate(&cfg, &app);
        assert_eq!(res.decision, EligibilityDecision::ManualReview);
        assert!(res.manual_review_reasons.iter().any(|r| r.code == DPD_RECENT));
    }

    #[test]
    fn test_dpd_24m_limit_auto_reject() {
        let cfg = default_config();
        let mut app = sample_applicant();
        app.dpd_24m = Some(45); // > 30 max
        let res = evaluate(&cfg, &app);
        assert_eq!(res.decision, EligibilityDecision::AutoReject);
        assert!(res.reject_reasons.iter().any(|r| r.code == DPD_EXCEEDS_LIMIT));
    }

    #[test]
    fn test_suit_filed_auto_reject() {
        let cfg = default_config();
        let mut app = sample_applicant();
        app.suit_filed = true;
        let res = evaluate(&cfg, &app);
        assert_eq!(res.decision, EligibilityDecision::AutoReject);
        assert!(res.reject_reasons.iter().any(|r| r.code == SUIT_FILED));
    }

    #[test]
    fn test_negative_landlord_reference_manual_review() {
        let cfg = default_config();
        let mut app = sample_applicant();
        app.landlord_reference_status = Some("negative".into());
        let res = evaluate(&cfg, &app);
        assert_eq!(res.decision, EligibilityDecision::ManualReview);
        assert!(res.manual_review_reasons.iter().any(|r| r.code == LANDLORD_REFERENCE_NEGATIVE));
    }

    #[test]
    fn test_co_applicant_needed_when_income_insufficient() {
        let cfg = default_config();
        let mut app = sample_applicant();
        // Income 20k * 3x = 60k < deposit 100k
        app.monthly_income = 20_000.0;
        app.deposit_amount = 100_000.0;
        app.monthly_rent = 6_000.0;
        app.emi_total = Some(0.0);
        let res = evaluate(&cfg, &app);
        assert_eq!(res.decision, EligibilityDecision::ActionRequired);
        assert!(res.action_required_reasons.iter().any(|r| r.code == "CO_APPLICANT_NEEDED"));
    }

    #[test]
    fn test_co_applicant_income_boosts_approval() {
        let cfg = default_config();
        let mut app = sample_applicant();
        app.monthly_income = 20_000.0;
        app.deposit_amount = 100_000.0;
        app.monthly_rent = 6_000.0;
        app.emi_total = Some(0.0);
        // Co-applicant adds 20k -> combined 40k * 3x = 120k > 100k deposit
        app.co_applicant_income = Some(20_000.0);
        let res = evaluate(&cfg, &app);
        assert_eq!(res.decision, EligibilityDecision::AutoApprove);
    }
}

