use serde::{Deserialize, Serialize};

// ─── Issue 94: Bank Statement Analysis DTOs ────────────────────────────────

/// Response body for a completed bank-statement analysis
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BankStatementAnalysisResponse {
    pub id: uuid::Uuid,
    pub zero_deposit_id: uuid::Uuid,

    pub avg_monthly_credit: Option<f64>,
    pub avg_monthly_debit: Option<f64>,
    pub net_monthly_cashflow: Option<f64>,
    pub salary_credit_count: Option<i32>,
    pub irregular_credit_flag: bool,
    pub bounced_cheque_count: i32,
    pub high_debt_load_flag: bool,

    pub debt_to_income_ratio: Option<f64>,
    pub cash_flow_stability_score: Option<f64>,

    /// Array of flag strings: ["BOUNCED_CHEQUE", "IRREGULAR_INCOME", ...]
    pub flags: serde_json::Value,

    pub statement_url: Option<String>,
    pub analysis_version: String,
    pub analyzed_at: chrono::DateTime<chrono::Utc>,
}

// ─── Issue 93: Risk Score DTOs ────────────────────────────────────────────

/// A single weighted factor that contributed to the risk score
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScoreFactor {
    /// Machine-readable signal code, e.g. "BOUNCED_CHEQUE"
    pub code: String,
    /// Human-readable label, e.g. "Bounced cheques detected"
    pub label: String,
    /// Detailed explanation for display/compliance
    pub detail: String,
    /// Direction: "POSITIVE" or "NEGATIVE" impact on risk
    pub direction: String,
    /// Weight of this factor in the final score (0.0–1.0)
    pub weight: f64,
}

/// Response body for a risk score
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LoanRiskScoreResponse {
    pub id: uuid::Uuid,
    pub zero_deposit_id: uuid::Uuid,

    /// 0.0 (very safe) to 1.0 (very likely to default)
    pub probability_of_default: f64,
    /// "LOW" | "MEDIUM" | "HIGH" | "VERY_HIGH"
    pub risk_tier: String,

    pub income_stability_score: Option<f64>,
    pub cashflow_score: Option<f64>,
    pub emi_burden_score: Option<f64>,

    pub score_factors: Vec<ScoreFactor>,

    pub model_version: String,
    pub scored_at: chrono::DateTime<chrono::Utc>,
}

// ─── Issue 95: Decision Reason DTOs ────────────────────────────────────────

/// One structured reason attached to an approve / reject decision
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DecisionReason {
    /// Machine code: e.g. "HIGH_RISK_SCORE", "BOUNCED_CHEQUES", "ADMIN_OVERRIDE"
    pub code: String,
    /// Short human label shown in the UI
    pub label: String,
    /// Full human-readable explanation (required for RBI compliance)
    pub detail: String,
}

/// Request body extensions for approve / reject that include decision_reasons
#[derive(Debug, Deserialize)]
pub struct DecisionReasonsPayload {
    /// Structured reasons list. May be empty for legacy callers (defaults to auto-derived reasons).
    #[serde(default)]
    pub decision_reasons: Vec<DecisionReason>,
}
