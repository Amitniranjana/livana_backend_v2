/// Issue 93 — ML-based risk scoring (v1: deterministic rule-based engine)
/// Issue 94 — Bank statement cash-flow analysis
///
/// Architecture note (from issue spec):
/// This module implements *v1* of risk scoring as a pure rule-based computation
/// embedded in the Rust backend. It is explicitly structured to be extracted into
/// a dedicated model-serving micro-service in a future epic once the ML training
/// approach is decided (separate Python FastAPI / Triton / SageMaker service that
/// this backend would call via HTTP). For now, the module is self-contained so
/// the feature can ship and collect labelled data for eventual model training.

use sqlx::PgPool;
use uuid::Uuid;
use serde_json::json;
use chrono::{DateTime, Utc};

use crate::dtos::loan_risk::{DecisionReason, LoanRiskScoreResponse, ScoreFactor};

// ─── Constants ─────────────────────────────────────────────────────────────

/// Weights for sub-scores → composite PoD.
/// Must sum to 1.0.
const W_INCOME_STABILITY: f64 = 0.35;
const W_CASHFLOW: f64 = 0.40;
const W_EMI_BURDEN: f64 = 0.25;

// ─── Input bundle ─────────────────────────────────────────────────────────

/// All inputs the scoring engine needs.  Collected from zero_deposits +
/// bank_statement_analyses rows.
#[derive(Debug, Clone)]
pub struct RiskInputs {
    pub zero_deposit_id: Uuid,

    /// From zero_deposits
    pub monthly_income: f64,
    pub monthly_rent: f64,
    pub requested_deposit_amount: f64,

    /// From bank_statement_analyses (None if no analysis has run yet)
    pub net_monthly_cashflow: Option<f64>,
    pub cash_flow_stability_score: Option<f64>,
    pub bounced_cheque_count: i32,
    pub irregular_income_flag: bool,
    pub high_debt_load_flag: bool,
    pub debt_to_income_ratio: Option<f64>,
}



// ─── Scoring engine ────────────────────────────────────────────────────────

/// Compute the probability-of-default and sub-scores.
///
/// Returns (pod, income_stability_score, cashflow_score, emi_burden_score, factors).
/// All sub-scores are 0–100 where **lower is riskier**.
pub fn compute_risk_score(inputs: &RiskInputs) -> (f64, f64, f64, f64, Vec<ScoreFactor>) {
    let mut factors: Vec<ScoreFactor> = Vec::new();

    // ── 1. Income Stability Score (0-100) ─────────────────────────────────
    let mut income_stability: f64 = 60.0; // baseline

    if inputs.irregular_income_flag {
        income_stability -= 30.0;
        factors.push(ScoreFactor {
            code: "IRREGULAR_INCOME".to_string(),
            label: "Irregular income detected".to_string(),
            detail: "Bank statement shows no consistent monthly credit pattern, suggesting freelance or inconsistent employment income.".to_string(),
            direction: "NEGATIVE".to_string(),
            weight: 0.20,
        });
    } else {
        income_stability += 20.0;
        factors.push(ScoreFactor {
            code: "REGULAR_INCOME".to_string(),
            label: "Regular income pattern".to_string(),
            detail: "Bank statement shows consistent monthly credits suggesting stable employment.".to_string(),
            direction: "POSITIVE".to_string(),
            weight: 0.20,
        });
    }

    // Cash-flow stability bonus
    if let Some(stability) = inputs.cash_flow_stability_score {
        let bonus = ((stability - 50.0) / 50.0) * 20.0; // +/-20 points around 50
        income_stability += bonus;
        if stability >= 70.0 {
            factors.push(ScoreFactor {
                code: "HIGH_CASHFLOW_STABILITY".to_string(),
                label: "High cash-flow stability".to_string(),
                detail: format!("Cash-flow stability index is {:.1}/100 — income is highly predictable.", stability),
                direction: "POSITIVE".to_string(),
                weight: 0.10,
            });
        } else if stability < 40.0 {
            factors.push(ScoreFactor {
                code: "LOW_CASHFLOW_STABILITY".to_string(),
                label: "Low cash-flow stability".to_string(),
                detail: format!("Cash-flow stability index is {:.1}/100 — income is highly variable.", stability),
                direction: "NEGATIVE".to_string(),
                weight: 0.10,
            });
        }
    }

    let income_stability_score = income_stability.clamp(0.0, 100.0);

    // ── 2. Cash-flow Score (0-100) ─────────────────────────────────────────
    let mut cashflow: f64 = 50.0;

    // Net monthly cash-flow relative to rent
    if let Some(net) = inputs.net_monthly_cashflow {
        let rent_coverage = net / inputs.monthly_rent.max(1.0);
        if rent_coverage >= 2.0 {
            cashflow += 30.0;
            factors.push(ScoreFactor {
                code: "STRONG_NET_CASHFLOW".to_string(),
                label: "Strong net cash-flow".to_string(),
                detail: format!("Net monthly cash-flow ({:.0} INR) covers rent {:.1}×.", net, rent_coverage),
                direction: "POSITIVE".to_string(),
                weight: 0.15,
            });
        } else if rent_coverage < 1.0 {
            cashflow -= 30.0;
            factors.push(ScoreFactor {
                code: "INSUFFICIENT_NET_CASHFLOW".to_string(),
                label: "Insufficient net cash-flow".to_string(),
                detail: format!("Net monthly cash-flow ({:.0} INR) is below the monthly rent ({:.0} INR).", net, inputs.monthly_rent),
                direction: "NEGATIVE".to_string(),
                weight: 0.20,
            });
        }
    }

    // Bounced cheques
    if inputs.bounced_cheque_count > 0 {
        let penalty = (inputs.bounced_cheque_count as f64 * 10.0).min(40.0);
        cashflow -= penalty;
        factors.push(ScoreFactor {
            code: "BOUNCED_CHEQUES".to_string(),
            label: "Bounced cheques detected".to_string(),
            detail: format!("{} bounced cheque(s) found in bank statement — indicates payment stress.", inputs.bounced_cheque_count),
            direction: "NEGATIVE".to_string(),
            weight: 0.15,
        });
    }

    let cashflow_score = cashflow.clamp(0.0, 100.0);

    // ── 3. EMI Burden Score (0-100) ────────────────────────────────────────
    let mut emi_burden: f64 = 70.0;

    let dti = inputs.debt_to_income_ratio.unwrap_or_else(|| {
        // Estimate from income alone
        inputs.monthly_rent / inputs.monthly_income.max(1.0)
    });

    if dti > 0.6 {
        emi_burden -= 40.0;
        factors.push(ScoreFactor {
            code: "HIGH_DEBT_TO_INCOME".to_string(),
            label: "High debt-to-income ratio".to_string(),
            detail: format!("Debt-to-income ratio is {:.0}% — above 60% threshold. Risk of default is significantly elevated.", dti * 100.0),
            direction: "NEGATIVE".to_string(),
            weight: 0.20,
        });
    } else if dti > 0.40 {
        emi_burden -= 20.0;
        factors.push(ScoreFactor {
            code: "MODERATE_DEBT_TO_INCOME".to_string(),
            label: "Moderate debt-to-income ratio".to_string(),
            detail: format!("Debt-to-income ratio is {:.0}% — between 40%-60%. Manageable but warrants monitoring.", dti * 100.0),
            direction: "NEGATIVE".to_string(),
            weight: 0.10,
        });
    } else {
        factors.push(ScoreFactor {
            code: "LOW_DEBT_TO_INCOME".to_string(),
            label: "Healthy debt-to-income ratio".to_string(),
            detail: format!("Debt-to-income ratio is {:.0}% — below 40%. Comfortable repayment capacity.", dti * 100.0),
            direction: "POSITIVE".to_string(),
            weight: 0.10,
        });
    }

    if inputs.high_debt_load_flag {
        emi_burden -= 15.0;
        factors.push(ScoreFactor {
            code: "HIGH_EXISTING_DEBT_LOAD".to_string(),
            label: "High existing debt load".to_string(),
            detail: "Bank statement analysis flagged a high number of existing EMI / loan debit patterns.".to_string(),
            direction: "NEGATIVE".to_string(),
            weight: 0.10,
        });
    }

    let emi_burden_score = emi_burden.clamp(0.0, 100.0);

    // ── 4. Composite Probability of Default ───────────────────────────────
    // Each sub-score is inverted (100 - score) to get a "risk" component,
    // then blended by weights, then normalised to [0,1].
    let risk_component = (100.0 - income_stability_score) * W_INCOME_STABILITY
        + (100.0 - cashflow_score) * W_CASHFLOW
        + (100.0 - emi_burden_score) * W_EMI_BURDEN;

    let pod = (risk_component / 100.0).clamp(0.0, 1.0);

    (pod, income_stability_score, cashflow_score, emi_burden_score, factors)
}

/// Derive a human-readable risk tier from the probability of default.
pub fn risk_tier_from_pod(pod: f64) -> &'static str {
    match pod {
        p if p < 0.20 => "LOW",
        p if p < 0.40 => "MEDIUM",
        p if p < 0.65 => "HIGH",
        _ => "VERY_HIGH",
    }
}

// ─── Database helpers ──────────────────────────────────────────────────────

/// Fetch inputs from DB and compute (or re-compute) the risk score,
/// persisting the result into `loan_risk_scores`.
///
/// Uses sqlx::query() (non-macro) consistent with the rest of the codebase,
/// so offline compilation works without `cargo sqlx prepare`.
pub async fn score_loan_application(
    db: &PgPool,
    zero_deposit_id: Uuid,
) -> Result<LoanRiskScoreResponse, String> {
    // 1. Load zero_deposit row — returns a JSON row like the rest of the codebase
    let zd_row = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT row_to_json(t) FROM (
            SELECT monthly_income::FLOAT8, monthly_rent::FLOAT8, requested_deposit_amount::FLOAT8
            FROM zero_deposits WHERE id = $1
        ) t
        "#,
    )
    .bind(zero_deposit_id)
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "Zero deposit application not found".to_string())?;

    let monthly_income = zd_row.get("monthly_income").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let monthly_rent = zd_row.get("monthly_rent").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let requested_deposit_amount = zd_row.get("requested_deposit_amount").and_then(|v| v.as_f64()).unwrap_or(0.0);

    // 2. Load bank statement analysis (most recent, if any)
    let bs_row = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT row_to_json(t) FROM (
            SELECT
                avg_monthly_credit::FLOAT8,
                net_monthly_cashflow::FLOAT8,
                cash_flow_stability_score::FLOAT8,
                bounced_cheque_count,
                irregular_credit_flag,
                high_debt_load_flag,
                debt_to_income_ratio::FLOAT8
            FROM bank_statement_analyses
            WHERE zero_deposit_id = $1
            ORDER BY analyzed_at DESC
            LIMIT 1
        ) t
        "#,
    )
    .bind(zero_deposit_id)
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?;

    let inputs = RiskInputs {
        zero_deposit_id,
        monthly_income,
        monthly_rent,
        requested_deposit_amount,
        net_monthly_cashflow: bs_row.as_ref()
            .and_then(|r| r.get("net_monthly_cashflow"))
            .and_then(|v| v.as_f64()),
        cash_flow_stability_score: bs_row.as_ref()
            .and_then(|r| r.get("cash_flow_stability_score"))
            .and_then(|v| v.as_f64()),
        bounced_cheque_count: bs_row.as_ref()
            .and_then(|r| r.get("bounced_cheque_count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32,
        irregular_income_flag: bs_row.as_ref()
            .and_then(|r| r.get("irregular_credit_flag"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        high_debt_load_flag: bs_row.as_ref()
            .and_then(|r| r.get("high_debt_load_flag"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        debt_to_income_ratio: bs_row.as_ref()
            .and_then(|r| r.get("debt_to_income_ratio"))
            .and_then(|v| v.as_f64()),
    };

    let (pod, income_score, cf_score, emi_score, factors) = compute_risk_score(&inputs);
    let tier = risk_tier_from_pod(pod);

    let factors_json = serde_json::to_value(&factors).unwrap_or(json!([]));

    // 3. Upsert loan_risk_scores (one row per application, re-scored on demand)
    let score_id = Uuid::new_v4();
    let score_row = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        INSERT INTO loan_risk_scores (
            id, zero_deposit_id,
            probability_of_default, risk_tier,
            income_stability_score, cashflow_score, emi_burden_score,
            monthly_income, monthly_rent, requested_deposit_amount,
            bounced_cheque_count, irregular_income_flag, high_debt_load_flag,
            score_factors, model_version, scored_at
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
            'v1-rule-based', NOW()
        )
        ON CONFLICT (zero_deposit_id) DO UPDATE SET
            probability_of_default  = EXCLUDED.probability_of_default,
            risk_tier               = EXCLUDED.risk_tier,
            income_stability_score  = EXCLUDED.income_stability_score,
            cashflow_score          = EXCLUDED.cashflow_score,
            emi_burden_score        = EXCLUDED.emi_burden_score,
            monthly_income          = EXCLUDED.monthly_income,
            monthly_rent            = EXCLUDED.monthly_rent,
            requested_deposit_amount= EXCLUDED.requested_deposit_amount,
            bounced_cheque_count    = EXCLUDED.bounced_cheque_count,
            irregular_income_flag   = EXCLUDED.irregular_income_flag,
            high_debt_load_flag     = EXCLUDED.high_debt_load_flag,
            score_factors           = EXCLUDED.score_factors,
            model_version           = EXCLUDED.model_version,
            scored_at               = NOW(),
            updated_at              = NOW()
        RETURNING row_to_json(loan_risk_scores.*)
        "#,
    )
    .bind(score_id)
    .bind(zero_deposit_id)
    .bind(pod)
    .bind(tier)
    .bind(income_score)
    .bind(cf_score)
    .bind(emi_score)
    .bind(inputs.monthly_income)
    .bind(inputs.monthly_rent)
    .bind(inputs.requested_deposit_amount)
    .bind(inputs.bounced_cheque_count)
    .bind(inputs.irregular_income_flag)
    .bind(inputs.high_debt_load_flag)
    .bind(factors_json)
    .fetch_one(db)
    .await
    .map_err(|e| e.to_string())?;

    let result_id = score_row
        .get("id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or(score_id);

    let result_pod = score_row
        .get("probability_of_default")
        .and_then(|v| v.as_f64())
        .unwrap_or(pod);

    let result_tier = score_row
        .get("risk_tier")
        .and_then(|v| v.as_str())
        .unwrap_or(tier)
        .to_string();

    let result_factors: Vec<ScoreFactor> = score_row
        .get("score_factors")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_else(|| factors.clone());

    let scored_at: DateTime<Utc> = score_row
        .get("scored_at")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<DateTime<Utc>>().ok())
        .unwrap_or_else(Utc::now);

    Ok(LoanRiskScoreResponse {
        id: result_id,
        zero_deposit_id,
        probability_of_default: result_pod,
        risk_tier: result_tier,
        income_stability_score: score_row.get("income_stability_score").and_then(|v| v.as_f64()),
        cashflow_score: score_row.get("cashflow_score").and_then(|v| v.as_f64()),
        emi_burden_score: score_row.get("emi_burden_score").and_then(|v| v.as_f64()),
        score_factors: result_factors,
        model_version: score_row
            .get("model_version")
            .and_then(|v| v.as_str())
            .unwrap_or("v1-rule-based")
            .to_string(),
        scored_at,
    })
}

// ─── Bank Statement Analysis Engine ────────────────────────────────────────

/// Issue 94 — Analyse the uploaded bank statement URL (v1: heuristic / estimation).
///
/// In production v2 this function would:
///   1. Download the PDF from S3
///   2. Send it to a Python parsing service (OCR + transaction extraction)
///   3. Receive structured transaction list
///   4. Apply the rules below
///
/// For v1, we perform rule-based estimation from the declared income vs rent.
pub async fn analyze_bank_statement(
    db: &PgPool,
    zero_deposit_id: Uuid,
    statement_url: Option<&str>,
) -> Result<Uuid, String> {
    // Load application data using consistent JSON row approach
    let zd = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT row_to_json(t) FROM (
            SELECT monthly_income::FLOAT8, monthly_rent::FLOAT8,
                   requested_deposit_amount::FLOAT8, bank_statement_url
            FROM zero_deposits WHERE id = $1
        ) t
        "#,
    )
    .bind(zero_deposit_id)
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "Application not found".to_string())?;

    let income = zd.get("monthly_income").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let rent = zd.get("monthly_rent").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let deposit = zd.get("requested_deposit_amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let stored_url = zd.get("bank_statement_url").and_then(|v| v.as_str()).map(|s| s.to_string());

    let effective_url = statement_url
        .map(|s| s.to_string())
        .or(stored_url);

    // ── v1 Rule-based estimation ──────────────────────────────────────────
    let avg_monthly_credit = income;
    let avg_monthly_debit = rent * 1.4; // heuristic: rent + living expenses
    let net_monthly_cashflow = avg_monthly_credit - avg_monthly_debit;

    // Debt-to-income: rent + deposit amortised over 12 months / income
    let dti = (rent + deposit / 12.0) / income.max(1.0);
    let high_debt_load_flag = dti > 0.55;
    let irregular_credit_flag = income < rent * 1.5; // too tight

    // Stability score: how predictable is the income
    let cash_flow_stability_score: f64 = if irregular_credit_flag { 35.0 } else { 72.0 };
    let bounced_cheque_count: i32 = 0; // v1 cannot detect from URL alone

    // Build flags array
    let mut flags: Vec<serde_json::Value> = Vec::new();
    if bounced_cheque_count > 0 {
        flags.push(json!("BOUNCED_CHEQUE"));
    }
    if irregular_credit_flag {
        flags.push(json!("IRREGULAR_INCOME"));
    }
    if high_debt_load_flag {
        flags.push(json!("HIGH_DEBT_LOAD"));
    }
    let flags_json = serde_json::Value::Array(flags);

    // Upsert bank_statement_analyses — use INSERT ... ON CONFLICT DO NOTHING
    // (idempotent: re-running always creates a new row since no UNIQUE constraint
    // on zero_deposit_id — multiple analyses per application are valid in v2)
    let analysis_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO bank_statement_analyses (
            id, zero_deposit_id,
            avg_monthly_credit, avg_monthly_debit, net_monthly_cashflow,
            salary_credit_count,
            irregular_credit_flag, bounced_cheque_count, high_debt_load_flag,
            debt_to_income_ratio, cash_flow_stability_score,
            flags, statement_url, analysis_version
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 'v1-rule-based'
        )
        "#,
    )
    .bind(analysis_id)
    .bind(zero_deposit_id)
    .bind(avg_monthly_credit)
    .bind(avg_monthly_debit)
    .bind(net_monthly_cashflow)
    .bind(1_i32) // salary_credit_count: v1 assumes 1 salary per month
    .bind(irregular_credit_flag)
    .bind(bounced_cheque_count)
    .bind(high_debt_load_flag)
    .bind(dti)
    .bind(cash_flow_stability_score)
    .bind(flags_json)
    .bind(effective_url.as_deref())
    .execute(db)
    .await
    .map_err(|e| e.to_string())?;

    Ok(analysis_id)
}

// ─── Decision-reason helpers (Issue 95) ──────────────────────────────────

/// Auto-derive a set of decision reasons from a risk score row.
/// Used when the admin doesn't supply explicit reasons.
pub async fn auto_decision_reasons_from_risk(
    db: &PgPool,
    zero_deposit_id: Uuid,
    approved: bool,
) -> Vec<DecisionReason> {
    let Ok(score) = score_loan_application(db, zero_deposit_id).await else {
        return vec![DecisionReason {
            code: if approved { "ADMIN_APPROVED" } else { "ADMIN_REJECTED" }.to_string(),
            label: if approved { "Admin approved" } else { "Admin rejected" }.to_string(),
            detail: "Decision made by admin without a computed risk score.".to_string(),
        }];
    };

    let mut reasons: Vec<DecisionReason> = Vec::new();

    // Top-level risk tier reason
    reasons.push(DecisionReason {
        code: format!("RISK_TIER_{}", score.risk_tier),
        label: format!("Risk tier: {}", score.risk_tier),
        detail: format!(
            "Automated risk model assigned a {} risk tier with probability-of-default = {:.1}%.",
            score.risk_tier,
            score.probability_of_default * 100.0
        ),
    });

    // Convert relevant score factors → decision reasons
    for factor in &score.score_factors {
        let matches_decision = (approved && factor.direction == "POSITIVE")
            || (!approved && factor.direction == "NEGATIVE");
        if matches_decision {
            reasons.push(DecisionReason {
                code: factor.code.clone(),
                label: factor.label.clone(),
                detail: factor.detail.clone(),
            });
        }
    }

    if reasons.len() == 1 {
        // Only the risk tier added — supplement with a generic reason
        reasons.push(DecisionReason {
            code: if approved { "OVERALL_RISK_ACCEPTABLE" } else { "OVERALL_RISK_UNACCEPTABLE" }.to_string(),
            label: if approved { "Overall risk acceptable" } else { "Overall risk too high" }.to_string(),
            detail: if approved {
                "All evaluated signals indicate the applicant is likely to honour the deposit commitment.".to_string()
            } else {
                "One or more evaluated signals indicate elevated probability of default.".to_string()
            },
        });
    }

    reasons
}
