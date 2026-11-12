//! Issue 110 — Eligibility Rules Engine (Deterministic v2)
//!
//! "The rules engine decides; the AI only supplies data and flags. AI signals NEVER
//! reject on their own — only the auto-reject rules or an admin can reject, and every
//! rejection carries reason codes the tenant can see. Tenants qualify on rent-paying
//! reliability, not loan capacity; upfront affordability is intentionally NOT checked."
//!
//! Check order:
//!   1. KYC & fraud signals
//!   2. Bureau pull (score, red flags, enquiries, existing EMIs, card dues)
//!   3. Income & employment verification
//!   4. Bank statement analysis, last 6 months
//!   5. Rental behaviour & co-applicant checks
//!   6. Tier assignment & token issuance decision
//!
//! Thresholds are read from `zd_ai_config`. Rules version: "elig-2026-09".

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::services::zd_token_service;
use crate::services::zd_verification_service::{
    self, VerificationCheckResult, VerificationStatusTriState,
    REASON_SELFIE_MISSING, REASON_ID_PHOTO_MISSING,
    REASON_FACE_MISMATCH, REASON_KYC_INCOMPLETE, PENDING_ACTION_COMPLETE_KYC,
    REASON_NO_DOCUMENTS_ANALYZED, REASON_DOC_TAMPER_SUSPECTED, REASON_DECLARED_VALUE_MISSING,
    REASON_LIVENESS_NOT_RUN, REASON_EXTRACTION_MISSING,
    REASON_PROVIDER_ERROR, REASON_TIMEOUT,
};

pub const RULES_VERSION: &str = "elig-2026-09";

// ---------------------------------------------------------------------------
// Auto-reject Reason Codes
// ---------------------------------------------------------------------------
pub const KYC_MISMATCH: &str = "KYC_MISMATCH";
pub const AGE_OUT_OF_RANGE: &str = "AGE_OUT_OF_RANGE";
pub const NOT_INDIAN_RESIDENT: &str = "NOT_INDIAN_RESIDENT";
pub const EMPLOYER_VERIFICATION_FAILED: &str = "EMPLOYER_VERIFICATION_FAILED";
pub const BOUNCE_LAST_3M: &str = "BOUNCE_LAST_3M";
pub const CIBIL_BELOW_650: &str = "CIBIL_BELOW_650";
pub const WRITE_OFF_OR_SETTLED: &str = "WRITE_OFF_OR_SETTLED";
pub const INCOME_BELOW_MIN_NO_COAPPLICANT: &str = "INCOME_BELOW_MIN_NO_COAPPLICANT";
pub const PRIOR_LIVANA_DEFAULT: &str = "PRIOR_LIVANA_DEFAULT";

// ---------------------------------------------------------------------------
// Rules Reason Codes (Manual Review & Action Required)
// ---------------------------------------------------------------------------
pub const TIER_C_REVIEW: &str = "TIER_C_REVIEW";
pub const RENT_ABOVE_TIER_CAP: &str = "RENT_ABOVE_TIER_CAP";
pub const DPD_RECENT: &str = "DPD_RECENT";
pub const HIGH_ENQUIRIES: &str = "HIGH_ENQUIRIES";
pub const NTC_ALT_SCORING: &str = "NTC_ALT_SCORING";
pub const LANDLORD_REFERENCE_NEGATIVE: &str = "LANDLORD_REFERENCE_NEGATIVE";
pub const KYC_INCOMPLETE: &str = "KYC_INCOMPLETE";
pub const FACE_MISMATCH: &str = "FACE_MISMATCH";
pub const ADMIN_OVERRIDE_INPUTS: &str = "ADMIN_OVERRIDE_INPUTS";
pub const COMPLETE_KYC: &str = "complete_kyc";

// ---------------------------------------------------------------------------
// AI Reason Codes (Flags from AI Callback)
// ---------------------------------------------------------------------------
pub const IRREGULAR_INCOME: &str = "IRREGULAR_INCOME";
pub const LOW_AVG_BALANCE: &str = "LOW_AVG_BALANCE";
pub const FREQUENT_OVERDRAFT: &str = "FREQUENT_OVERDRAFT";
pub const UNEXPLAINED_CASH: &str = "UNEXPLAINED_CASH";
pub const MULTIPLE_LOAN_APPS: &str = "MULTIPLE_LOAN_APPS";
pub const INCOME_MISMATCH: &str = "INCOME_MISMATCH";
pub const UNDECLARED_EMI: &str = "UNDECLARED_EMI";
pub const DOC_TAMPER_SUSPECTED: &str = "DOC_TAMPER_SUSPECTED";
pub const DUPLICATE_BANK_ACCOUNT: &str = "DUPLICATE_BANK_ACCOUNT";
pub const DEVICE_VELOCITY: &str = "DEVICE_VELOCITY";
pub const DOC_UNREADABLE: &str = "DOC_UNREADABLE";

// ---------------------------------------------------------------------------
// System Reason Codes
// ---------------------------------------------------------------------------
pub const AI_UNAVAILABLE: &str = "AI_UNAVAILABLE";

// ---------------------------------------------------------------------------
// Enums & Structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentDecision {
    AutoApprove,
    ActionRequired,
    ManualReview,
    AutoReject,
}

pub type EligibilityDecision = AssessmentDecision;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TenantTier {
    A,
    B,
    C,
}

impl TenantTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            TenantTier::A => "A",
            TenantTier::B => "B",
            TenantTier::C => "C",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "A" | "a" => Some(TenantTier::A),
            "B" | "b" => Some(TenantTier::B),
            "C" | "c" => Some(TenantTier::C),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssessmentCheckItem {
    pub check: String,
    pub result: String, // "pass" | "review" | "fail" | "not_evaluated"
    pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<serde_json::Value>,
}

impl Default for AssessmentCheckItem {
    fn default() -> Self {
        Self {
            check: String::new(),
            result: String::new(),
            detail: String::new(),
            reason: None,
            score: None,
            provenance: None,
        }
    }
}

impl AssessmentCheckItem {
    pub fn new(check: impl Into<String>, result: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            check: check.into(),
            result: result.into(),
            detail: detail.into(),
            reason: None,
            score: None,
            provenance: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesEngineResult {
    pub decision: AssessmentDecision,
    pub tier: Option<TenantTier>,
    pub recommended_token_amount: f64,
    pub income_considered: f64,
    pub checks: Vec<AssessmentCheckItem>,
    pub reason_codes: Vec<String>,
    pub ai_flags: Vec<String>,
    pub pending_actions: Vec<String>,
    pub admin_summary: String,
    pub tenant_message: String,
    pub rules_version: String,
}

// ---------------------------------------------------------------------------
// Engine Inputs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ApplicationInput {
    pub application_id: Uuid,
    pub monthly_rent: f64,
    pub employment_type: String, // "salaried" | "self_employed" | "gig" | "student"
    pub employer_name: Option<String>,
    pub declared_monthly_income: f64,
    pub variable_pay_monthly: Option<f64>,
    pub current_job_start_date: Option<NaiveDate>,
    pub total_experience_months: Option<i32>,
    pub business_start_date: Option<NaiveDate>,
    pub is_repeat_renter: bool,
    pub created_at: DateTime<Utc>,
}

pub type ApplicationSignals = ApplicationInput;

#[derive(Debug, Clone)]
pub struct KycInput {
    pub name_on_id: String,
    pub dob: Option<NaiveDate>,
    pub is_indian_resident: bool,
    pub has_aadhaar: bool,
    pub has_pan: bool,
    pub mobile_linked_to_aadhaar: bool,
    pub address_proof_verified: bool,
    pub bank_account_matches_name: bool,
    pub kyc_mismatch: bool,

    // Tri-state checks (Fail-Closed)
    pub face_match: VerificationCheckResult,
    pub liveness: VerificationCheckResult,
    pub doc_extraction: VerificationCheckResult,
    pub tamper_check: VerificationCheckResult,
    pub name_match: VerificationCheckResult,
    pub dob_match: VerificationCheckResult,
    pub pan_match: VerificationCheckResult,
    pub aadhaar_match: VerificationCheckResult,

    // Source tracking
    pub input_source: String, // "provider", "mock", "admin_override_inputs"
}

impl KycInput {
    pub fn default_valid() -> Self {
        Self {
            name_on_id: "Aman Verma".into(),
            dob: Some(NaiveDate::from_ymd_opt(1996, 5, 10).unwrap()),
            is_indian_resident: true,
            has_aadhaar: true,
            has_pan: true,
            mobile_linked_to_aadhaar: true,
            address_proof_verified: true,
            bank_account_matches_name: true,
            kyc_mismatch: false,
            face_match: VerificationCheckResult::pass("face_match", "98.5% match", Some(98.5)),
            liveness: VerificationCheckResult::pass("liveness", "Liveness confirmed", None),
            doc_extraction: VerificationCheckResult::pass("doc_extraction", "Clean extraction", None),
            tamper_check: VerificationCheckResult::pass("tamper_check", "Documents clean", None),
            name_match: VerificationCheckResult::pass("name_match", "Name matches", Some(100.0)),
            dob_match: VerificationCheckResult::pass("dob_match", "DOB matches", Some(100.0)),
            pan_match: VerificationCheckResult::pass("pan_match", "PAN matches", Some(100.0)),
            aadhaar_match: VerificationCheckResult::pass("aadhaar_match", "Aadhaar matches", Some(100.0)),
            input_source: "provider".into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BureauInput {
    pub cibil_score: Option<i32>,
    pub report_date: Option<NaiveDate>,
    pub active_emi_total: f64,
    pub card_dues_total: f64,
    pub write_offs: i32,
    pub settlements: i32,
    pub suit_filed: bool,
    pub max_dpd_12m: i32,
    pub max_dpd_24m: i32,
    pub enquiries_3m: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractedBankData {
    pub net_take_home_monthly: Option<f64>,
    pub variable_pay_monthly: Option<f64>,
    pub variable_pay_months_seen: i32,
    pub avg_monthly_credits_6m: Option<f64>,
    pub salary_regular: Option<bool>,
    pub bounces_3m: i32,
    pub avg_monthly_balance: f64,
    pub overdraft_days_6m: i32,
    pub unexplained_cash_max_month: f64,
    pub loan_app_count: i32,
    pub emi_debits_monthly: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct EmployerVerifInput {
    pub status: String, // "pending" | "verified" | "failed"
}

#[derive(Debug, Clone)]
pub struct CoApplicantInput {
    pub status: String, // "invited" | "accepted" | "declined"
    pub income: f64,
    pub cibil_score: Option<i32>,
    pub emi_total: f64,
    pub invited_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LandlordRefInput {
    pub status: String, // "pending" | "verified" | "negative" | "unreachable"
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ZdAiConfig {
    pub auto_issue_enabled: bool,
    pub age_min: i32,
    pub age_max: i32,
    pub min_tenancy_months: i32,
    pub deposit_rent_multiple: f64,
    pub fee_flat_amount: f64,
    pub max_obligations_pct: f64,
    pub max_rent_pct: f64,
    pub min_cibil_score: i32,
    pub co_applicant_min_cibil: i32,
    pub co_applicant_window_days: i64,
    pub max_enquiries_3m: i32,
    pub tier_a_max_rent: f64,
    pub tier_b_max_rent: f64,
    pub tier_c_max_rent: f64,
    pub max_overdraft_days_6m: i32,
    pub max_unexplained_cash_pct: f64,
    pub max_loan_apps: i32,
    pub face_match_min_confidence: f32,
}

impl Default for ZdAiConfig {
    fn default() -> Self {
        Self {
            auto_issue_enabled: true,
            age_min: 21,
            age_max: 55,
            min_tenancy_months: 11,
            deposit_rent_multiple: 3.0,
            fee_flat_amount: 2000.0,
            max_obligations_pct: 50.0,
            max_rent_pct: 35.0,
            min_cibil_score: 650,
            co_applicant_min_cibil: 700,
            co_applicant_window_days: 7,
            max_enquiries_3m: 4,
            tier_a_max_rent: 40000.0,
            tier_b_max_rent: 25000.0,
            tier_c_max_rent: 15000.0,
            max_overdraft_days_6m: 5,
            max_unexplained_cash_pct: 50.0,
            max_loan_apps: 2,
            face_match_min_confidence: 85.0,
        }
    }
}

impl ZdAiConfig {
    pub fn from_json(v: &Value) -> Self {
        let mut cfg = Self::default();
        if let Some(b) = v.get("auto_issue_enabled").and_then(|x| x.as_bool()) {
            cfg.auto_issue_enabled = b;
        }
        if let Some(f) = v.get("face_match_min_confidence").and_then(|x| x.as_f64()) {
            cfg.face_match_min_confidence = f as f32;
        }
        if let Some(a) = v.get("age_min").and_then(|x| x.as_i64()) {
            cfg.age_min = a as i32;
        }
        if let Some(a) = v.get("age_max").and_then(|x| x.as_i64()) {
            cfg.age_max = a as i32;
        }
        if let Some(m) = v.get("max_obligations_pct").and_then(|x| x.as_f64()) {
            cfg.max_obligations_pct = m;
        }
        if let Some(r) = v.get("max_rent_pct").and_then(|x| x.as_f64()) {
            cfg.max_rent_pct = r;
        }
        if let Some(s) = v.get("min_cibil_score").and_then(|x| x.as_i64()) {
            cfg.min_cibil_score = s as i32;
        }
        if let Some(s) = v.get("co_applicant_min_cibil").and_then(|x| x.as_i64()) {
            cfg.co_applicant_min_cibil = s as i32;
        }
        if let Some(e) = v.get("max_enquiries_3m").and_then(|x| x.as_i64()) {
            cfg.max_enquiries_3m = e as i32;
        }

        // Tiers
        if let Some(tiers) = v.get("tiers") {
            if let Some(a) = tiers.get("A").and_then(|x| x.get("max_rent")).and_then(|x| x.as_f64()) {
                cfg.tier_a_max_rent = a;
            }
            if let Some(b) = tiers.get("B").and_then(|x| x.get("max_rent")).and_then(|x| x.as_f64()) {
                cfg.tier_b_max_rent = b;
            }
            if let Some(c) = tiers.get("C").and_then(|x| x.get("max_rent")).and_then(|x| x.as_f64()) {
                cfg.tier_c_max_rent = c;
            }
        }

        // Bank checks
        if let Some(bc) = v.get("bank_checks") {
            if let Some(o) = bc.get("max_overdraft_days_6m").and_then(|x| x.as_i64()) {
                cfg.max_overdraft_days_6m = o as i32;
            }
            if let Some(u) = bc.get("max_unexplained_cash_pct_of_income").and_then(|x| x.as_f64()) {
                cfg.max_unexplained_cash_pct = u;
            }
            if let Some(l) = bc.get("max_loan_apps").and_then(|x| x.as_i64()) {
                cfg.max_loan_apps = l as i32;
            }
        }

        cfg
    }
}

pub async fn load_config_from_db(pool: &PgPool) -> ZdAiConfig {
    let row: Option<Value> = sqlx::query_scalar("SELECT config FROM zd_ai_config LIMIT 1")
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

    match row {
        Some(v) => ZdAiConfig::from_json(&v),
        None => ZdAiConfig::default(),
    }
}

// ---------------------------------------------------------------------------
// Pure Deterministic Rules Evaluation
// ---------------------------------------------------------------------------

pub fn evaluate_rules(
    config: &ZdAiConfig,
    app: &ApplicationInput,
    kyc: &KycInput,
    bureau: &BureauInput,
    extracted: &ExtractedBankData,
    ai_flags: &[String],
    employer_verif: Option<&EmployerVerifInput>,
    co_applicant: Option<&CoApplicantInput>,
    landlord_ref: Option<&LandlordRefInput>,
    prior_livana_default: bool,
    days_since_application: i64,
) -> RulesEngineResult {
    let mut checks: Vec<AssessmentCheckItem> = Vec::new();
    let mut auto_reject_reasons: Vec<String> = Vec::new();
    let mut manual_review_reasons: Vec<String> = Vec::new();
    let mut action_required_reasons: Vec<String> = Vec::new();
    let mut pending_actions: Vec<String> = Vec::new();

    // ────────────────────────────────────────────────────────────────────────
    // Step 1: KYC & Fraud
    // ────────────────────────────────────────────────────────────────────────
    
    // Check 1.1: Name Match
    checks.push(AssessmentCheckItem {
        check: "name_match".into(),
        result: kyc.name_match.result.as_str().into(),
        detail: kyc.name_match.detail.clone(),
        reason: kyc.name_match.reason.clone(),
        score: kyc.name_match.score,
        provenance: kyc.name_match.provenance.clone(),
    });
    if kyc.name_match.result == VerificationStatusTriState::Fail {
        auto_reject_reasons.push(KYC_MISMATCH.into());
    }

    // Check 1.2: Date of Birth Match
    checks.push(AssessmentCheckItem {
        check: "dob_match".into(),
        result: kyc.dob_match.result.as_str().into(),
        detail: kyc.dob_match.detail.clone(),
        reason: kyc.dob_match.reason.clone(),
        score: kyc.dob_match.score,
        provenance: kyc.dob_match.provenance.clone(),
    });
    if kyc.dob_match.result == VerificationStatusTriState::Fail {
        auto_reject_reasons.push(KYC_MISMATCH.into());
    }

    // Check 1.3: PAN Match
    checks.push(AssessmentCheckItem {
        check: "pan_match".into(),
        result: kyc.pan_match.result.as_str().into(),
        detail: kyc.pan_match.detail.clone(),
        reason: kyc.pan_match.reason.clone(),
        score: kyc.pan_match.score,
        provenance: kyc.pan_match.provenance.clone(),
    });
    if kyc.pan_match.result == VerificationStatusTriState::Fail {
        auto_reject_reasons.push(KYC_MISMATCH.into());
    }

    // Check 1.4: Aadhaar Match
    checks.push(AssessmentCheckItem {
        check: "aadhaar_match".into(),
        result: kyc.aadhaar_match.result.as_str().into(),
        detail: kyc.aadhaar_match.detail.clone(),
        reason: kyc.aadhaar_match.reason.clone(),
        score: kyc.aadhaar_match.score,
        provenance: kyc.aadhaar_match.provenance.clone(),
    });
    if kyc.aadhaar_match.result == VerificationStatusTriState::Fail {
        auto_reject_reasons.push(KYC_MISMATCH.into());
    }

    // Check 1.5: Selfie Liveness
    checks.push(AssessmentCheckItem {
        check: "selfie_liveness".into(),
        result: kyc.liveness.result.as_str().into(),
        detail: kyc.liveness.detail.clone(),
        reason: kyc.liveness.reason.clone(),
        score: kyc.liveness.score,
        provenance: kyc.liveness.provenance.clone(),
    });
    if kyc.liveness.result == VerificationStatusTriState::Fail {
        manual_review_reasons.push("LIVENESS_FAILED".into());
    }

    // Check 1.6: Face Match
    checks.push(AssessmentCheckItem {
        check: "face_match".into(),
        result: kyc.face_match.result.as_str().into(),
        detail: kyc.face_match.detail.clone(),
        reason: kyc.face_match.reason.clone(),
        score: kyc.face_match.score,
        provenance: kyc.face_match.provenance.clone(),
    });
    if kyc.face_match.result == VerificationStatusTriState::Review || kyc.face_match.result == VerificationStatusTriState::Fail {
        manual_review_reasons.push(FACE_MISMATCH.into());
    }

    // Missing selfie / ID photo / incomplete KYC => action_required with complete_kyc
    let is_missing_selfie_or_id = kyc.face_match.result == VerificationStatusTriState::NotEvaluated
        && (kyc.face_match.reason.as_deref() == Some(REASON_SELFIE_MISSING)
            || kyc.face_match.reason.as_deref() == Some(REASON_ID_PHOTO_MISSING)
            || kyc.liveness.result == VerificationStatusTriState::NotEvaluated
            || !kyc.has_aadhaar || !kyc.has_pan);

    if is_missing_selfie_or_id {
        action_required_reasons.push(KYC_INCOMPLETE.into());
        pending_actions.push(PENDING_ACTION_COMPLETE_KYC.into());
    } else if kyc.face_match.result == VerificationStatusTriState::NotEvaluated {
        manual_review_reasons.push(KYC_INCOMPLETE.into());
    }

    // Check 1.7: Document Tamper Check
    checks.push(AssessmentCheckItem {
        check: "tamper_check".into(),
        result: kyc.tamper_check.result.as_str().into(),
        detail: kyc.tamper_check.detail.clone(),
        reason: kyc.tamper_check.reason.clone(),
        score: kyc.tamper_check.score,
        provenance: kyc.tamper_check.provenance.clone(),
    });
    if kyc.tamper_check.result == VerificationStatusTriState::Review {
        manual_review_reasons.push(DOC_TAMPER_SUSPECTED.into());
    }

    // Check 1.8: Indian Resident
    if !kyc.is_indian_resident {
        auto_reject_reasons.push(NOT_INDIAN_RESIDENT.into());
        checks.push(AssessmentCheckItem::new(
            "indian_resident",
            "fail",
            "Applicant is not an Indian resident",
        ));
    } else {
        checks.push(AssessmentCheckItem::new(
            "indian_resident",
            "pass",
            "Applicant confirmed Indian resident",
        ));
    }

    // Check 1.9: Age calculation
    let age = kyc.dob.map(|dob| {
        let now = Utc::now().date_naive();
        now.year() - dob.year()
    });

    match age {
        Some(a) if a < config.age_min || a > config.age_max => {
            auto_reject_reasons.push(AGE_OUT_OF_RANGE.into());
            checks.push(AssessmentCheckItem {
                check: "age_range".into(),
                result: "fail".into(),
                detail: format!("Age {} is outside permitted range {}-{}", a, config.age_min, config.age_max),
                reason: Some(AGE_OUT_OF_RANGE.into()),
                score: None,
                provenance: None,
            });
        }
        Some(a) => {
            checks.push(AssessmentCheckItem {
                check: "age_range".into(),
                result: "pass".into(),
                detail: format!("Age {} is within permitted range {}-{}", a, config.age_min, config.age_max),
                reason: None,
                score: None,
                provenance: None,
            });
        }
        None => {
            checks.push(AssessmentCheckItem {
                check: "age_range".into(),
                result: "not_evaluated".into(),
                detail: "Date of birth not declared; age could not be calculated".into(),
                reason: Some(REASON_DECLARED_VALUE_MISSING.into()),
                score: None,
                provenance: None,
            });
            manual_review_reasons.push(KYC_INCOMPLETE.into());
        }
    }

    // ────────────────────────────────────────────────────────────────────────
    // Step 2: Bureau Checks
    // ────────────────────────────────────────────────────────────────────────
    if bureau.write_offs > 0 || bureau.settlements > 0 || bureau.suit_filed {
        auto_reject_reasons.push(WRITE_OFF_OR_SETTLED.into());
        checks.push(AssessmentCheckItem::new(
            "bureau_adverse_records",
            "fail",
            format!(
                "Write-offs: {}, Settlements: {}, Suit filed: {}",
                bureau.write_offs, bureau.settlements, bureau.suit_filed
            ),
        ));
    } else {
        checks.push(AssessmentCheckItem::new(
            "bureau_adverse_records",
            "pass",
            "No write-offs, settlements, or suits filed",
        ));
    }

    if bureau.max_dpd_12m >= 30 || bureau.max_dpd_24m >= 60 {
        manual_review_reasons.push(DPD_RECENT.into());
        checks.push(AssessmentCheckItem::new(
            "dpd_recent",
            "review",
            format!("Max DPD 12m: {}, 24m: {}", bureau.max_dpd_12m, bureau.max_dpd_24m),
        ));
    } else {
        checks.push(AssessmentCheckItem::new(
            "dpd_recent",
            "pass",
            format!("DPD records clean (12m: {}, 24m: {})", bureau.max_dpd_12m, bureau.max_dpd_24m),
        ));
    }

    if bureau.enquiries_3m > config.max_enquiries_3m {
        manual_review_reasons.push(HIGH_ENQUIRIES.into());
        checks.push(AssessmentCheckItem::new(
            "bureau_enquiries",
            "review",
            format!("{} enquiries in last 3 months (max: {})", bureau.enquiries_3m, config.max_enquiries_3m),
        ));
    } else {
        checks.push(AssessmentCheckItem::new(
            "bureau_enquiries",
            "pass",
            format!("{} enquiries in last 3 months (within limit)", bureau.enquiries_3m),
        ));
    }

    // CIBIL Score & Tier Determination
    let (computed_tier, is_ntc) = match bureau.cibil_score {
        Some(score) if score >= 750 => {
            // Tier A if salaried with 1+ yr in current job (or total experience >= 12)
            let is_experienced_salaried = app.employment_type == "salaried"
                && (app.total_experience_months.unwrap_or(0) >= 12
                    || app.current_job_start_date.map(|d| {
                        let now = Utc::now().date_naive();
                        (now.year() - d.year()) * 12 + (now.month() as i32 - d.month() as i32) >= 12
                    }).unwrap_or(false));

            if is_experienced_salaried {
                (Some(TenantTier::A), false)
            } else {
                (Some(TenantTier::B), false)
            }
        }
        Some(score) if score >= 700 => (Some(TenantTier::B), false),
        Some(score) if score >= 650 => (Some(TenantTier::C), false),
        Some(score) if score > 0 => {
            auto_reject_reasons.push(CIBIL_BELOW_650.into());
            checks.push(AssessmentCheckItem::new(
                "cibil_score",
                "fail",
                format!("CIBIL score {} is below minimum threshold 650", score),
            ));
            (None, false)
        }
        _ => {
            // Score is None or 0 -> New-To-Credit (NTC)
            (Some(TenantTier::C), true)
        }
    };

    if is_ntc {
        manual_review_reasons.push(NTC_ALT_SCORING.into());
        checks.push(AssessmentCheckItem::new(
            "cibil_score",
            "review",
            "New-to-credit (NTC) applicant routed to alternative scoring",
        ));
    } else if let Some(score) = bureau.cibil_score {
        if score >= 650 {
            checks.push(AssessmentCheckItem::new(
                "cibil_score",
                "pass",
                format!("CIBIL score: {}", score),
            ));
        }
    }

    // ────────────────────────────────────────────────────────────────────────
    // Step 3: Income & Employment Verification
    // ────────────────────────────────────────────────────────────────────────
    let (mut income_considered, min_income_mult) = match app.employment_type.as_str() {
        "salaried" => {
            let take_home = extracted.net_take_home_monthly.unwrap_or(app.declared_monthly_income);
            let var_pay = app.variable_pay_monthly.unwrap_or(0.0);
            let var_pay_considered = if extracted.variable_pay_months_seen >= 6 {
                var_pay
            } else {
                var_pay * 0.50
            };
            (take_home + var_pay_considered, 3.0)
        }
        "self_employed" => {
            let credits = extracted.avg_monthly_credits_6m.unwrap_or(app.declared_monthly_income);
            (credits * 0.50, 3.0)
        }
        "gig" => {
            let credits = extracted.avg_monthly_credits_6m.unwrap_or(app.declared_monthly_income);
            (credits * 0.50, 3.5)
        }
        "student" => {
            let co_inc = co_applicant.and_then(|c| if c.status == "accepted" { Some(c.income) } else { None }).unwrap_or(0.0);
            (co_inc, 3.0)
        }
        _ => (app.declared_monthly_income, 3.0),
    };

    let mut total_obligations = app.monthly_rent + bureau.active_emi_total + bureau.card_dues_total;
    let min_required_income = app.monthly_rent * min_income_mult;

    let income_below_min = income_considered < min_required_income;
    let obligations_exceeded = income_considered > 0.0 && (total_obligations / income_considered) > (config.max_obligations_pct / 100.0);
    let rent_exceeded = income_considered > 0.0 && (app.monthly_rent / income_considered) > (config.max_rent_pct / 100.0);

    let needs_co_applicant = income_below_min || obligations_exceeded || rent_exceeded || computed_tier == Some(TenantTier::C);

    if needs_co_applicant {
        match co_applicant {
            Some(co) if co.status == "accepted" => {
                let co_cibil_ok = co.cibil_score.map(|s| s >= config.co_applicant_min_cibil).unwrap_or(false);
                if co_cibil_ok {
                    income_considered += co.income;
                    total_obligations += co.emi_total;
                    checks.push(AssessmentCheckItem::new(
                        "co_applicant",
                        "pass",
                        format!("Co-applicant accepted with CIBIL {:?}", co.cibil_score),
                    ));
                } else {
                    checks.push(AssessmentCheckItem::new(
                        "co_applicant",
                        "fail",
                        format!("Co-applicant CIBIL {:?} is below required {}", co.cibil_score, config.co_applicant_min_cibil),
                    ));
                    if days_since_application > config.co_applicant_window_days {
                        auto_reject_reasons.push(INCOME_BELOW_MIN_NO_COAPPLICANT.into());
                    } else {
                        action_required_reasons.push("co_applicant".into());
                        pending_actions.push("co_applicant".into());
                    }
                }
            }
            _ => {
                if days_since_application > config.co_applicant_window_days {
                    auto_reject_reasons.push(INCOME_BELOW_MIN_NO_COAPPLICANT.into());
                    checks.push(AssessmentCheckItem::new(
                        "co_applicant",
                        "fail",
                        format!("Co-applicant not provided within {} days", config.co_applicant_window_days),
                    ));
                } else {
                    action_required_reasons.push("co_applicant".into());
                    pending_actions.push("co_applicant".into());
                    checks.push(AssessmentCheckItem::new(
                        "co_applicant",
                        "review",
                        "Co-applicant required for qualification",
                    ));
                }
            }
        }
    }

    // Re-verify obligations with updated income_considered
    let final_obligations_pct = if income_considered > 0.0 { (total_obligations / income_considered) * 100.0 } else { 100.0 };
    let final_rent_pct = if income_considered > 0.0 { (app.monthly_rent / income_considered) * 100.0 } else { 100.0 };

    checks.push(AssessmentCheckItem::new(
        "obligations_50pct",
        if final_obligations_pct <= config.max_obligations_pct { "pass" } else { "review" },
        format!("₹{:.0} of ₹{:.0} max ({:.1}%)", total_obligations, income_considered * 0.50, final_obligations_pct),
    ));

    checks.push(AssessmentCheckItem::new(
        "rent_35pct",
        if final_rent_pct <= config.max_rent_pct { "pass" } else { "review" },
        format!("₹{:.0} is {:.1}% of income (max {}%)", app.monthly_rent, final_rent_pct, config.max_rent_pct),
    ));

    // Employer Verification Check
    match employer_verif.map(|v| v.status.as_str()) {
        Some("verified") => {
            checks.push(AssessmentCheckItem::new(
                "employer_verification",
                "pass",
                "Employer verified via official email OTP / HR",
            ));
        }
        Some("failed") => {
            auto_reject_reasons.push(EMPLOYER_VERIFICATION_FAILED.into());
            checks.push(AssessmentCheckItem::new(
                "employer_verification",
                "fail",
                "Official employer verification failed",
            ));
        }
        _ => {
            // Missing or pending
            action_required_reasons.push("employer_email".into());
            pending_actions.push("employer_email".into());
            checks.push(AssessmentCheckItem::new(
                "employer_verification",
                "review",
                "Employer email OTP verification pending",
            ));
        }
    }

    // ────────────────────────────────────────────────────────────────────────
    // Step 4: Bank Statement Analysis (last 6 months)
    // ────────────────────────────────────────────────────────────────────────
    if extracted.bounces_3m > 0 {
        auto_reject_reasons.push(BOUNCE_LAST_3M.into());
        checks.push(AssessmentCheckItem::new(
            "bank_bounces",
            "fail",
            format!("{} cheque/NACH bounce(s) in last 3 months", extracted.bounces_3m),
        ));
    } else {
        checks.push(AssessmentCheckItem::new(
            "bank_bounces",
            "pass",
            "Zero cheque or NACH bounces in last 3 months",
        ));
    }

    if extracted.salary_regular == Some(false) {
        manual_review_reasons.push(IRREGULAR_INCOME.into());
    }

    if extracted.avg_monthly_balance < app.monthly_rent {
        manual_review_reasons.push(LOW_AVG_BALANCE.into());
    }

    if extracted.overdraft_days_6m > config.max_overdraft_days_6m {
        manual_review_reasons.push(FREQUENT_OVERDRAFT.into());
    }

    if income_considered > 0.0 && extracted.unexplained_cash_max_month > (income_considered * (config.max_unexplained_cash_pct / 100.0)) {
        manual_review_reasons.push(UNEXPLAINED_CASH.into());
    }

    if extracted.loan_app_count > config.max_loan_apps {
        manual_review_reasons.push(MULTIPLE_LOAN_APPS.into());
    }

    // ────────────────────────────────────────────────────────────────────────
    // Step 5: Rental Behaviour
    // ────────────────────────────────────────────────────────────────────────
    if prior_livana_default {
        auto_reject_reasons.push(PRIOR_LIVANA_DEFAULT.into());
        checks.push(AssessmentCheckItem::new(
            "prior_default",
            "fail",
            "Applicant defaulted on an earlier Zero Deposit agreement",
        ));
    }

    if app.is_repeat_renter || computed_tier == Some(TenantTier::C) {
        match landlord_ref.map(|l| l.status.as_str()) {
            Some("negative") => {
                manual_review_reasons.push(LANDLORD_REFERENCE_NEGATIVE.into());
                checks.push(AssessmentCheckItem::new(
                    "landlord_reference",
                    "review",
                    "Negative landlord reference reported",
                ));
            }
            Some("verified") => {
                checks.push(AssessmentCheckItem::new(
                    "landlord_reference",
                    "pass",
                    "Landlord reference verified",
                ));
            }
            _ => {
                action_required_reasons.push("landlord_reference".into());
                pending_actions.push("landlord_reference".into());
                checks.push(AssessmentCheckItem::new(
                    "landlord_reference",
                    "review",
                    "Previous landlord reference or rent receipts required",
                ));
            }
        }
    }

    // ────────────────────────────────────────────────────────────────────────
    // Step 6: AI Flags Routing
    // ────────────────────────────────────────────────────────────────────────
    for flag in ai_flags {
        manual_review_reasons.push(flag.clone());
    }

    // ────────────────────────────────────────────────────────────────────────
    // Step 7: Tier Assignment & Rent Cap
    // ────────────────────────────────────────────────────────────────────────
    if computed_tier == Some(TenantTier::C) {
        manual_review_reasons.push(TIER_C_REVIEW.into());
    }

    let tier_max_rent = match computed_tier {
        Some(TenantTier::A) => config.tier_a_max_rent,
        Some(TenantTier::B) => config.tier_b_max_rent,
        Some(TenantTier::C) => config.tier_c_max_rent,
        None => 0.0,
    };

    if app.monthly_rent > tier_max_rent && computed_tier.is_some() {
        manual_review_reasons.push(RENT_ABOVE_TIER_CAP.into());
        checks.push(AssessmentCheckItem::new(
            "tier_rent_cap",
            "review",
            format!("Rent ₹{:.0} exceeds Tier {:?} cap ₹{:.0}", app.monthly_rent, computed_tier.unwrap(), tier_max_rent),
        ));
    } else if computed_tier.is_some() {
        checks.push(AssessmentCheckItem::new(
            "tier_rent_cap",
            "pass",
            format!("Rent ₹{:.0} within Tier {:?} cap ₹{:.0}", app.monthly_rent, computed_tier.unwrap(), tier_max_rent),
        ));
    }

    // ────────────────────────────────────────────────────────────────────────
    // Final Decision Synthesis
    // ────────────────────────────────────────────────────────────────────────
    let recommended_token_amount = app.monthly_rent * config.deposit_rent_multiple;

    let has_not_evaluated_checks = checks.iter().any(|c| c.result == "not_evaluated");
    let is_untrusted_input_source = kyc.input_source == "mock" || kyc.input_source == "admin_override_inputs";

    let (decision, mut combined_reasons) = if !auto_reject_reasons.is_empty() {
        (AssessmentDecision::AutoReject, auto_reject_reasons)
    } else if !action_required_reasons.is_empty() {
        (AssessmentDecision::ActionRequired, action_required_reasons)
    } else if has_not_evaluated_checks {
        let mut r = manual_review_reasons;
        r.push(KYC_INCOMPLETE.into());
        (AssessmentDecision::ManualReview, r)
    } else if is_untrusted_input_source {
        let mut r = manual_review_reasons;
        if kyc.input_source == "admin_override_inputs" {
            r.push(ADMIN_OVERRIDE_INPUTS.into());
        } else {
            r.push("MOCK_INPUTS_REVIEW".into());
        }
        (AssessmentDecision::ManualReview, r)
    } else if !config.auto_issue_enabled {
        let mut r = manual_review_reasons;
        r.push("AUTO_ISSUE_DISABLED".into());
        (AssessmentDecision::ManualReview, r)
    } else if !manual_review_reasons.is_empty() {
        (AssessmentDecision::ManualReview, manual_review_reasons)
    } else {
        (AssessmentDecision::AutoApprove, Vec::new())
    };

    // Deduplicate reasons
    combined_reasons.sort();
    combined_reasons.dedup();
    pending_actions.sort();
    pending_actions.dedup();

    let tenant_message = match decision {
        AssessmentDecision::AutoApprove => "Your monthly income comfortably covers the rent. Tokens issued!".to_string(),
        AssessmentDecision::ActionRequired => {
            if pending_actions.contains(&PENDING_ACTION_COMPLETE_KYC.to_string()) {
                "Action required: please complete your KYC and upload your selfie and ID photo.".to_string()
            } else {
                "Action required: please provide the requested verification or documents.".to_string()
            }
        }
        AssessmentDecision::ManualReview => "Your application is under standard review by our underwriting team.".to_string(),
        AssessmentDecision::AutoReject => "Based on our underwriting criteria, your application could not be approved at this time.".to_string(),
    };

    let admin_summary = format!(
        "Decision: {:?}, Tier: {:?}, Income: ₹{:.0}, Rent: ₹{:.0}, Tokens: ₹{:.0}, Reason codes: {:?}",
        decision, computed_tier, income_considered, app.monthly_rent, recommended_token_amount, combined_reasons
    );

    RulesEngineResult {
        decision,
        tier: computed_tier,
        recommended_token_amount,
        income_considered,
        checks,
        reason_codes: combined_reasons,
        ai_flags: ai_flags.to_vec(),
        pending_actions,
        admin_summary,
        tenant_message,
        rules_version: RULES_VERSION.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Database Integration Helpers
// ---------------------------------------------------------------------------

pub async fn run_rules_engine_for_application(
    pool: &PgPool,
    app_id: Uuid,
    extracted: &ExtractedBankData,
    ai_flags: &[String],
) -> Result<RulesEngineResult, sqlx::Error> {
    let config = load_config_from_db(pool).await;

    // 1. Fetch application row
    let app_row = sqlx::query(
        r#"SELECT
            id, user_id,
            monthly_rent::FLOAT8 as monthly_rent,
            COALESCE(deposit_amount::FLOAT8, requested_deposit_amount::FLOAT8, monthly_rent * 3.0) as deposit_amount,
            COALESCE(monthly_income::FLOAT8, 0.0) as declared_monthly_income,
            COALESCE(variable_pay_monthly::FLOAT8, 0.0) as variable_pay_monthly,
            COALESCE(employment_type, 'salaried') as employment_type,
            employer_name,
            current_job_start_date,
            total_experience_months,
            business_start_date,
            is_repeat_renter,
            bureau_summary,
            created_at
        FROM zero_deposits WHERE id = $1"#
    )
    .bind(app_id)
    .fetch_one(pool)
    .await?;

    let user_id: Uuid = app_row.get("user_id");
    let monthly_rent: f64 = app_row.get("monthly_rent");
    let declared_monthly_income: f64 = app_row.get("declared_monthly_income");
    let variable_pay_monthly: Option<f64> = app_row.try_get("variable_pay_monthly").ok();
    let employment_type: String = app_row.get("employment_type");
    let employer_name: Option<String> = app_row.try_get("employer_name").ok();
    let current_job_start_date: Option<NaiveDate> = app_row.try_get("current_job_start_date").ok();
    let total_experience_months: Option<i32> = app_row.try_get("total_experience_months").ok();
    let business_start_date: Option<NaiveDate> = app_row.try_get("business_start_date").ok();
    let is_repeat_renter: bool = app_row.get("is_repeat_renter");
    let created_at: DateTime<Utc> = app_row.get("created_at");
    let bureau_json: Value = app_row.try_get("bureau_summary").unwrap_or(Value::Null);

    let app_input = ApplicationInput {
        application_id: app_id,
        monthly_rent,
        employment_type,
        employer_name,
        declared_monthly_income,
        variable_pay_monthly,
        current_job_start_date,
        total_experience_months,
        business_start_date,
        is_repeat_renter,
        created_at,
    };

    // 2. Fetch KYC
    let kyc_row = sqlx::query(
        r#"SELECT
            id, full_name, date_of_birth, profile_picture_url,
            govt_id_type, govt_id_number, govt_id_document_url, country,
            extracted_name, extracted_doc_number, extracted_dob,
            document_integrity_status::TEXT as document_integrity_status,
            face_match_score, face_match_status, face_match_reason,
            liveness_score, liveness_status,
            image_override_urls
        FROM kyc_submissions WHERE user_id = $1 LIMIT 1"#
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    let kyc_input = match kyc_row {
        Some(r) => {
            let full_name: String = r.get("full_name");
            let dob_str: Option<String> = r.try_get("date_of_birth").ok();
            let dob = dob_str.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
            let country: Option<String> = r.try_get("country").ok();
            let is_indian = country.map(|c| c.to_lowercase() == "india" || c.to_lowercase() == "in").unwrap_or(true);

            let profile_picture_url: Option<String> = r.try_get("profile_picture_url").ok();
            let govt_id_type: String = r.try_get("govt_id_type").unwrap_or_default();
            let govt_id_number: String = r.try_get("govt_id_number").unwrap_or_default();
            let govt_id_document_url: Option<String> = r.try_get("govt_id_document_url").ok();

            let extracted_name: Option<String> = r.try_get("extracted_name").ok();
            let extracted_doc_number: Option<String> = r.try_get("extracted_doc_number").ok();
            let extracted_dob: Option<NaiveDate> = r.try_get("extracted_dob").ok();
            let doc_integrity: Option<String> = r.try_get("document_integrity_status").ok();

            let face_match_score: Option<f64> = r.try_get("face_match_score").ok();
            let face_match_reason: Option<String> = r.try_get("face_match_reason").ok();
            let liveness_score: Option<f64> = r.try_get("liveness_score").ok();
            let liveness_status: Option<String> = r.try_get("liveness_status").ok();
            let image_override_urls: Option<Value> = r.try_get("image_override_urls").ok();

            let input_source = if image_override_urls.is_some() && !image_override_urls.as_ref().unwrap().is_null() {
                "admin_override_inputs".to_string()
            } else {
                "production".to_string()
            };

            // Evaluate Face Match
            let face_match = if profile_picture_url.as_deref().unwrap_or("").trim().is_empty() {
                VerificationCheckResult::not_evaluated(
                    "face_match",
                    REASON_SELFIE_MISSING,
                    "Selfie photo is missing",
                )
            } else if govt_id_document_url.as_deref().unwrap_or("").trim().is_empty() {
                VerificationCheckResult::not_evaluated(
                    "face_match",
                    REASON_ID_PHOTO_MISSING,
                    "Government ID photo is missing",
                )
            } else if let Some(score) = face_match_score {
                let s = score as f32;
                if let Some(ref rsn) = face_match_reason {
                    if rsn == REASON_PROVIDER_ERROR || rsn == REASON_TIMEOUT {
                        VerificationCheckResult::not_evaluated(
                            "face_match",
                            rsn.as_str(),
                            "Provider error during face match evaluation",
                        )
                    } else if s >= config.face_match_min_confidence {
                        VerificationCheckResult::pass("face_match", "Faces match with high confidence", Some(s))
                    } else {
                        VerificationCheckResult::review(
                            "face_match",
                            REASON_FACE_MISMATCH,
                            format!("Face match score {:.1}% is below threshold {:.1}%", s, config.face_match_min_confidence),
                            Some(s),
                        )
                    }
                } else if s >= config.face_match_min_confidence {
                    VerificationCheckResult::pass("face_match", "Faces match with high confidence", Some(s))
                } else {
                    VerificationCheckResult::review(
                        "face_match",
                        REASON_FACE_MISMATCH,
                        format!("Face match score {:.1}% is below threshold {:.1}%", s, config.face_match_min_confidence),
                        Some(s),
                    )
                }
            } else {
                VerificationCheckResult::not_evaluated(
                    "face_match",
                    REASON_KYC_INCOMPLETE,
                    "Face match has not been computed yet",
                )
            };

            // Evaluate Liveness
            let liveness = if profile_picture_url.as_deref().unwrap_or("").trim().is_empty() {
                VerificationCheckResult::not_evaluated(
                    "liveness",
                    REASON_LIVENESS_NOT_RUN,
                    "Liveness check not run: selfie missing",
                )
            } else if let Some(status) = liveness_status {
                match status.to_lowercase().as_str() {
                    "passed" | "pass" => VerificationCheckResult::pass("liveness", "Liveness check passed", liveness_score.map(|s| s as f32)),
                    "failed" | "fail" => VerificationCheckResult::review(
                        "liveness",
                        "LIVENESS_FAILED",
                        "Liveness check failed",
                        liveness_score.map(|s| s as f32),
                    ),
                    _ => VerificationCheckResult::not_evaluated(
                        "liveness",
                        REASON_LIVENESS_NOT_RUN,
                        "Liveness status pending or unknown",
                    ),
                }
            } else {
                VerificationCheckResult::not_evaluated(
                    "liveness",
                    REASON_LIVENESS_NOT_RUN,
                    "Liveness check has not been run",
                )
            };

            // Evaluate Document Extraction
            let doc_extraction = if govt_id_document_url.as_deref().unwrap_or("").trim().is_empty() {
                VerificationCheckResult::not_evaluated(
                    "doc_extraction",
                    REASON_EXTRACTION_MISSING,
                    "Government ID photo is missing",
                )
            } else if extracted_name.is_none() && extracted_doc_number.is_none() && extracted_dob.is_none() {
                VerificationCheckResult::not_evaluated(
                    "doc_extraction",
                    REASON_EXTRACTION_MISSING,
                    "Document data extraction has not completed",
                )
            } else {
                VerificationCheckResult::pass("doc_extraction", "Document fields extracted", None)
            };

            // Evaluate Tamper Check
            let tamper_check = if govt_id_document_url.as_deref().unwrap_or("").trim().is_empty() {
                VerificationCheckResult::not_evaluated(
                    "tamper_check",
                    REASON_NO_DOCUMENTS_ANALYZED,
                    "No document uploaded for tamper analysis",
                )
            } else if let Some(status) = doc_integrity {
                match status.to_uppercase().as_str() {
                    "CLEAN" => VerificationCheckResult::pass("tamper_check", "No signs of tampering detected", None),
                    "SUSPICIOUS" | "FLAGGED" => VerificationCheckResult::review(
                        "tamper_check",
                        REASON_DOC_TAMPER_SUSPECTED,
                        format!("Document integrity status is {}", status),
                        None,
                    ),
                    _ => VerificationCheckResult::not_evaluated(
                        "tamper_check",
                        REASON_NO_DOCUMENTS_ANALYZED,
                        "Tamper check pending",
                    ),
                }
            } else {
                VerificationCheckResult::not_evaluated(
                    "tamper_check",
                    REASON_NO_DOCUMENTS_ANALYZED,
                    "Document tamper analysis not run",
                )
            };

            // Evaluate Field Matches via zd_verification_service
            let name_match = zd_verification_service::evaluate_name_match(Some(&full_name), extracted_name.as_deref());
            let dob_match = zd_verification_service::evaluate_dob_match(dob, extracted_dob);
            let pan_match = if govt_id_type.to_lowercase() == "pan" {
                zd_verification_service::evaluate_pan_match(Some(&govt_id_number), extracted_doc_number.as_deref())
            } else {
                VerificationCheckResult::not_evaluated(
                    "pan_match",
                    REASON_DECLARED_VALUE_MISSING,
                    "PAN not declared as government ID",
                )
            };
            let aadhaar_match = if govt_id_type.to_lowercase() == "aadhaar" {
                zd_verification_service::evaluate_aadhaar_match(Some(&govt_id_number), extracted_doc_number.as_deref())
            } else {
                VerificationCheckResult::not_evaluated(
                    "aadhaar_match",
                    REASON_DECLARED_VALUE_MISSING,
                    "Aadhaar not declared as government ID",
                )
            };

            let kyc_mismatch = name_match.result == VerificationStatusTriState::Fail || dob_match.result == VerificationStatusTriState::Fail;

            KycInput {
                name_on_id: full_name,
                dob,
                is_indian_resident: is_indian,
                has_aadhaar: govt_id_type.to_lowercase() == "aadhaar",
                has_pan: govt_id_type.to_lowercase() == "pan",
                mobile_linked_to_aadhaar: true,
                address_proof_verified: true,
                bank_account_matches_name: true,
                kyc_mismatch,
                face_match,
                liveness,
                doc_extraction,
                tamper_check,
                name_match,
                dob_match,
                pan_match,
                aadhaar_match,
                input_source,
            }
        }
        None => KycInput {
            name_on_id: "Applicant".into(),
            dob: None,
            is_indian_resident: true,
            has_aadhaar: false,
            has_pan: false,
            mobile_linked_to_aadhaar: false,
            address_proof_verified: false,
            bank_account_matches_name: false,
            kyc_mismatch: false,
            face_match: VerificationCheckResult::not_evaluated("face_match", REASON_SELFIE_MISSING, "No KYC or selfie uploaded"),
            liveness: VerificationCheckResult::not_evaluated("liveness", REASON_LIVENESS_NOT_RUN, "No selfie uploaded"),
            doc_extraction: VerificationCheckResult::not_evaluated("doc_extraction", REASON_EXTRACTION_MISSING, "No government ID uploaded"),
            tamper_check: VerificationCheckResult::not_evaluated("tamper_check", REASON_NO_DOCUMENTS_ANALYZED, "No documents uploaded"),
            name_match: VerificationCheckResult::not_evaluated("name_match", REASON_DECLARED_VALUE_MISSING, "KYC not submitted"),
            dob_match: VerificationCheckResult::not_evaluated("dob_match", REASON_DECLARED_VALUE_MISSING, "KYC not submitted"),
            pan_match: VerificationCheckResult::not_evaluated("pan_match", REASON_DECLARED_VALUE_MISSING, "KYC not submitted"),
            aadhaar_match: VerificationCheckResult::not_evaluated("aadhaar_match", REASON_DECLARED_VALUE_MISSING, "KYC not submitted"),
            input_source: "production".into(),
        },
    };

    // 3. Parse bureau
    let bureau_input = BureauInput {
        cibil_score: bureau_json.get("score").and_then(|v| v.as_i64()).map(|v| v as i32),
        report_date: bureau_json.get("report_date").and_then(|v| v.as_str()).and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
        active_emi_total: bureau_json.get("active_emi_total").and_then(|v| v.as_f64()).or_else(|| bureau_json.get("emi_total_monthly").and_then(|v| v.as_f64())).unwrap_or(0.0),
        card_dues_total: bureau_json.get("card_dues_total").and_then(|v| v.as_f64()).or_else(|| bureau_json.get("card_dues").and_then(|v| v.as_f64())).unwrap_or(0.0),
        write_offs: bureau_json.get("write_offs").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        settlements: bureau_json.get("settlements").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        suit_filed: bureau_json.get("suit_filed").and_then(|v| v.as_bool()).unwrap_or(false),
        max_dpd_12m: bureau_json.get("max_dpd_12m").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        max_dpd_24m: bureau_json.get("max_dpd_24m").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        enquiries_3m: bureau_json.get("enquiries_3m").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
    };

    // 4. Employer verification
    let emp_status = sqlx::query_scalar::<_, String>(
        "SELECT status::TEXT FROM zd_employer_verifications WHERE zero_deposit_id = $1 ORDER BY id DESC LIMIT 1"
    )
    .bind(app_id)
    .fetch_optional(pool)
    .await?;

    let emp_verif = emp_status.map(|st| EmployerVerifInput { status: st });

    // 5. Co-applicant
    let co_row = sqlx::query(
        "SELECT status::TEXT as status, declared_monthly_income::FLOAT8 as income, cibil_score, emi_total::FLOAT8 as emi_total, invited_at FROM zd_co_applicants WHERE zero_deposit_id = $1 ORDER BY id DESC LIMIT 1"
    )
    .bind(app_id)
    .fetch_optional(pool)
    .await?;

    let co_app = co_row.map(|r| CoApplicantInput {
        status: r.get("status"),
        income: r.try_get("income").unwrap_or(0.0),
        cibil_score: r.try_get::<i16, _>("cibil_score").ok().map(|s| s as i32),
        emi_total: r.try_get("emi_total").unwrap_or(0.0),
        invited_at: r.try_get("invited_at").unwrap_or_else(|_| Utc::now()),
    });

    // 6. Landlord reference
    let landlord_status = sqlx::query_scalar::<_, String>(
        "SELECT status::TEXT FROM zd_landlord_references WHERE zero_deposit_id = $1 ORDER BY id DESC LIMIT 1"
    )
    .bind(app_id)
    .fetch_optional(pool)
    .await?;

    let landlord_ref = landlord_status.map(|st| LandlordRefInput { status: st });

    // 7. Prior Livana default
    let prior_default = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM zero_deposits WHERE user_id = $1 AND id != $2 AND status = 'blocked'::zero_deposit_status)"
    )
    .bind(user_id)
    .bind(app_id)
    .fetch_one(pool)
    .await?;

    let days_since = (Utc::now() - created_at).num_days();

    Ok(evaluate_rules(
        &config,
        &app_input,
        &kyc_input,
        &bureau_input,
        extracted,
        ai_flags,
        emp_verif.as_ref(),
        co_app.as_ref(),
        landlord_ref.as_ref(),
        prior_default,
        days_since,
    ))
}

pub async fn persist_tier(
    pool: &PgPool,
    user_id: Uuid,
    tier: TenantTier,
    source: &str,
    reason: &str,
    changed_by: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO zd_tenant_tiers (user_id, tier, source, on_time_months, changed_by, reason, updated_at)
           VALUES ($1, $2::zd_tenant_tier, $3, 0, $4, $5, NOW())
           ON CONFLICT (user_id) WHERE user_id IS NOT NULL DO UPDATE
           SET tier = EXCLUDED.tier,
               source = EXCLUDED.source,
               changed_by = EXCLUDED.changed_by,
               reason = EXCLUDED.reason,
               updated_at = NOW()
           WHERE zd_tenant_tiers.source != 'admin' OR EXCLUDED.source = 'admin'"#
    )
    .bind(user_id)
    .bind(tier.as_str())
    .bind(source)
    .bind(changed_by)
    .bind(reason)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn persist_assessment(
    pool: &PgPool,
    app_id: Uuid,
    assessment_id: Option<Uuid>,
    model_version: &str,
    result: &RulesEngineResult,
    extracted: &Value,
    bureau_summary: &Value,
    admin_summary: &str,
) -> Result<Uuid, sqlx::Error> {
    // 1. Mark previous assessments for this application as is_current = false
    sqlx::query(
        "UPDATE zero_deposit_ai_assessments SET is_current = false WHERE zero_deposit_id = $1"
    )
    .bind(app_id)
    .execute(pool)
    .await?;

    let decision_str = match result.decision {
        AssessmentDecision::AutoApprove => "auto_approve",
        AssessmentDecision::ActionRequired => "action_required",
        AssessmentDecision::ManualReview => "manual_review",
        AssessmentDecision::AutoReject => "auto_reject",
    };

    let tier_str = result.tier.map(|t| t.as_str());
    let checks_val = serde_json::to_value(&result.checks).unwrap_or(Value::Array(vec![]));
    let reason_codes_val = serde_json::to_value(&result.reason_codes).unwrap_or(Value::Array(vec![]));
    let ai_flags_val = serde_json::to_value(&result.ai_flags).unwrap_or(Value::Array(vec![]));

    let row_id = match assessment_id {
        Some(aid) => {
            sqlx::query_scalar::<_, Uuid>(
                r#"INSERT INTO zero_deposit_ai_assessments (
                    id, assessment_id, zero_deposit_id, status, is_current, model_version, rules_version,
                    decision, tier, recommended_token_amount, income_considered, checks,
                    reason_codes, ai_flags, extracted, bureau_summary, admin_summary,
                    tenant_message, completed_at
                )
                VALUES (
                    $1, $1::TEXT, $2, 'completed', true, $3, $4,
                    $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, NOW()
                )
                ON CONFLICT (id) DO UPDATE
                SET status = 'completed',
                    is_current = true,
                    model_version = EXCLUDED.model_version,
                    rules_version = EXCLUDED.rules_version,
                    decision = EXCLUDED.decision,
                    tier = EXCLUDED.tier,
                    recommended_token_amount = EXCLUDED.recommended_token_amount,
                    income_considered = EXCLUDED.income_considered,
                    checks = EXCLUDED.checks,
                    reason_codes = EXCLUDED.reason_codes,
                    ai_flags = EXCLUDED.ai_flags,
                    extracted = EXCLUDED.extracted,
                    bureau_summary = EXCLUDED.bureau_summary,
                    admin_summary = EXCLUDED.admin_summary,
                    tenant_message = EXCLUDED.tenant_message,
                    completed_at = NOW()
                RETURNING id"#
            )
            .bind(aid)
            .bind(app_id)
            .bind(model_version)
            .bind(&result.rules_version)
            .bind(decision_str)
            .bind(tier_str)
            .bind(result.recommended_token_amount)
            .bind(result.income_considered)
            .bind(checks_val)
            .bind(reason_codes_val)
            .bind(ai_flags_val)
            .bind(extracted)
            .bind(bureau_summary)
            .bind(admin_summary)
            .bind(&result.tenant_message)
            .fetch_one(pool)
            .await?
        }
        None => {
            let gen_id = Uuid::new_v4();
            sqlx::query_scalar::<_, Uuid>(
                r#"INSERT INTO zero_deposit_ai_assessments (
                    id, assessment_id, zero_deposit_id, status, is_current, model_version, rules_version,
                    decision, tier, recommended_token_amount, income_considered, checks,
                    reason_codes, ai_flags, extracted, bureau_summary, admin_summary,
                    tenant_message, completed_at
                )
                VALUES (
                    $1, $1::TEXT, $2, 'completed', true, $3, $4,
                    $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, NOW()
                )
                RETURNING id"#
            )
            .bind(gen_id)
            .bind(app_id)
            .bind(model_version)
            .bind(&result.rules_version)
            .bind(decision_str)
            .bind(tier_str)
            .bind(result.recommended_token_amount)
            .bind(result.income_considered)
            .bind(checks_val)
            .bind(reason_codes_val)
            .bind(ai_flags_val)
            .bind(extracted)
            .bind(bureau_summary)
            .bind(admin_summary)
            .bind(&result.tenant_message)
            .fetch_one(pool)
            .await?
        }
    };

    Ok(row_id)
}

pub async fn apply_assessment_decision(
    pool: &PgPool,
    app_id: Uuid,
    user_id: Uuid,
    assessment_id: Uuid,
    result: &RulesEngineResult,
    monthly_rent: f64,
) -> Result<(), sqlx::Error> {
    match result.decision {
        AssessmentDecision::AutoApprove => {
            // Assign tier
            if let Some(tier) = result.tier {
                let _ = persist_tier(pool, user_id, tier, "assessment", "AI auto-approval", None).await;
            }

            // Issue tokens via zd_token_service
            let deposit_amount = monthly_rent * 3.0;
            let fee_amount = monthly_rent + 2000.0;

            zd_token_service::issue(
                pool,
                app_id,
                monthly_rent,
                Some("ai_auto"),
                None, // issued_by is None for AI auto-approval
                Some(assessment_id),
                Some("AI auto-approval tokens issued"),
            )
            .await
            .map_err(|e| sqlx::Error::Protocol(format!("Token issue failed: {}", e)))?;

            sqlx::query(
                "UPDATE zero_deposits SET status = 'tokens_issued'::zero_deposit_status, deposit_amount = $2, fee_amount = $3, updated_at = NOW() WHERE id = $1"
            )
            .bind(app_id)
            .bind(deposit_amount)
            .bind(fee_amount)
            .execute(pool)
            .await?;
        }
        AssessmentDecision::ActionRequired => {
            sqlx::query(
                "UPDATE zero_deposits SET status = 'action_required'::zero_deposit_status, pending_actions = $2, updated_at = NOW() WHERE id = $1"
            )
            .bind(app_id)
            .bind(&result.pending_actions)
            .execute(pool)
            .await?;
        }
        AssessmentDecision::ManualReview => {
            sqlx::query(
                "UPDATE zero_deposits SET status = 'ai_manual_review'::zero_deposit_status, updated_at = NOW() WHERE id = $1"
            )
            .bind(app_id)
            .execute(pool)
            .await?;
        }
        AssessmentDecision::AutoReject => {
            sqlx::query(
                "UPDATE zero_deposits SET status = 'rejected'::zero_deposit_status, updated_at = NOW() WHERE id = $1"
            )
            .bind(app_id)
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}

pub async fn run_and_apply_rules_engine(
    pool: &PgPool,
    app_id: Uuid,
) -> Result<RulesEngineResult, sqlx::Error> {
    let row = sqlx::query(
        "SELECT user_id, monthly_rent::FLOAT8 as rent FROM zero_deposits WHERE id = $1"
    )
    .bind(app_id)
    .fetch_one(pool)
    .await?;

    let user_id: Uuid = row.get("user_id");
    let monthly_rent: f64 = row.get("rent");

    // Fetch existing extracted data and ai_flags from the latest assessment, or fallback to defaults
    let prev_assessment = sqlx::query(
        r#"SELECT extracted, ai_flags, bureau_summary
           FROM zero_deposit_ai_assessments
           WHERE zero_deposit_id = $1 AND is_current = true
           ORDER BY created_at DESC LIMIT 1"#
    )
    .bind(app_id)
    .fetch_optional(pool)
    .await?;

    let (extracted, ai_flags, bureau_summary_val): (ExtractedBankData, Vec<String>, Value) = if let Some(r) = prev_assessment {
        let extracted_json = r.try_get::<Value, _>("extracted").unwrap_or(Value::Null);
        let flags_json = r.try_get::<Value, _>("ai_flags").unwrap_or(Value::Null);
        let bureau_val = r.try_get::<Value, _>("bureau_summary").unwrap_or(Value::Null);
        let ext = serde_json::from_value::<ExtractedBankData>(extracted_json).unwrap_or_default();
        let flags: Vec<String> = flags_json.as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        (ext, flags, bureau_val)
    } else {
        (ExtractedBankData::default(), vec![], Value::Null)
    };

    let result = run_rules_engine_for_application(pool, app_id, &extracted, &ai_flags).await?;

    let assessment_id = persist_assessment(
        pool,
        app_id,
        None,
        "elig-2026-09",
        &result,
        &serde_json::to_value(&extracted).unwrap_or(Value::Null),
        &bureau_summary_val,
        &result.admin_summary,
    )
    .await?;

    apply_assessment_decision(pool, app_id, user_id, assessment_id, &result, monthly_rent).await?;

    Ok(result)
}

// ---------------------------------------------------------------------------
// Unit Tests (Comprehensive Coverage)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn base_kyc() -> KycInput {
        KycInput::default_valid()
    }

    fn base_bureau() -> BureauInput {
        BureauInput {
            cibil_score: Some(735),
            report_date: Some(NaiveDate::from_ymd_opt(2026, 9, 29).unwrap()),
            active_emi_total: 4000.0,
            card_dues_total: 1500.0,
            write_offs: 0,
            settlements: 0,
            suit_filed: false,
            max_dpd_12m: 0,
            max_dpd_24m: 0,
            enquiries_3m: 1,
        }
    }

    fn base_extracted() -> ExtractedBankData {
        ExtractedBankData {
            net_take_home_monthly: Some(32000.0),
            variable_pay_monthly: Some(0.0),
            variable_pay_months_seen: 0,
            avg_monthly_credits_6m: Some(33500.0),
            salary_regular: Some(true),
            bounces_3m: 0,
            avg_monthly_balance: 14500.0,
            overdraft_days_6m: 0,
            unexplained_cash_max_month: 0.0,
            loan_app_count: 0,
            emi_debits_monthly: Some(4000.0),
        }
    }

    fn base_app() -> ApplicationInput {
        ApplicationInput {
            application_id: Uuid::new_v4(),
            monthly_rent: 10000.0,
            employment_type: "salaried".into(),
            employer_name: Some("TCS".into()),
            declared_monthly_income: 32000.0,
            variable_pay_monthly: Some(0.0),
            current_job_start_date: Some(NaiveDate::from_ymd_opt(2024, 6, 1).unwrap()),
            total_experience_months: Some(30),
            business_start_date: None,
            is_repeat_renter: false,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_worked_example_prompt_passes() {
        // Worked example from prompt:
        // rent ₹10,000; salaried, 2 yrs in job; take-home ₹32,000; EMI ₹4,000; card dues ₹1,500; CIBIL 735
        // -> income >= 3x rent (₹30,000): pass;
        // -> rent+EMI+card dues <= 50% (₹16,000): ₹15,500 pass;
        // -> rent <= 35% of income: 31% pass;
        // -> Tier B (700-749), cap ₹25,000 pass;
        // -> tokens issued ₹30,000
        let config = ZdAiConfig::default();
        let app = base_app();
        let kyc = base_kyc();
        let bureau = base_bureau();
        let extracted = base_extracted();
        let emp = EmployerVerifInput { status: "verified".into() };

        let res = evaluate_rules(
            &config,
            &app,
            &kyc,
            &bureau,
            &extracted,
            &[],
            Some(&emp),
            None,
            None,
            false,
            1,
        );

        assert_eq!(res.decision, AssessmentDecision::AutoApprove);
        assert_eq!(res.tier, Some(TenantTier::B));
        assert_eq!(res.recommended_token_amount, 30000.0);
        assert_eq!(res.income_considered, 32000.0);
        assert!(res.reason_codes.is_empty());
    }

    #[test]
    fn test_auto_reject_on_cibil_below_650() {
        let config = ZdAiConfig::default();
        let app = base_app();
        let kyc = base_kyc();
        let mut bureau = base_bureau();
        bureau.cibil_score = Some(610);
        let extracted = base_extracted();
        let emp = EmployerVerifInput { status: "verified".into() };

        let res = evaluate_rules(&config, &app, &kyc, &bureau, &extracted, &[], Some(&emp), None, None, false, 1);
        assert_eq!(res.decision, AssessmentDecision::AutoReject);
        assert!(res.reason_codes.contains(&CIBIL_BELOW_650.to_string()));
    }

    #[test]
    fn test_auto_reject_on_write_offs() {
        let config = ZdAiConfig::default();
        let app = base_app();
        let kyc = base_kyc();
        let mut bureau = base_bureau();
        bureau.write_offs = 1;
        let extracted = base_extracted();
        let emp = EmployerVerifInput { status: "verified".into() };

        let res = evaluate_rules(&config, &app, &kyc, &bureau, &extracted, &[], Some(&emp), None, None, false, 1);
        assert_eq!(res.decision, AssessmentDecision::AutoReject);
        assert!(res.reason_codes.contains(&WRITE_OFF_OR_SETTLED.to_string()));
    }

    #[test]
    fn test_auto_reject_on_bounce_last_3m() {
        let config = ZdAiConfig::default();
        let app = base_app();
        let kyc = base_kyc();
        let bureau = base_bureau();
        let mut extracted = base_extracted();
        extracted.bounces_3m = 1;
        let emp = EmployerVerifInput { status: "verified".into() };

        let res = evaluate_rules(&config, &app, &kyc, &bureau, &extracted, &[], Some(&emp), None, None, false, 1);
        assert_eq!(res.decision, AssessmentDecision::AutoReject);
        assert!(res.reason_codes.contains(&BOUNCE_LAST_3M.to_string()));
    }

    #[test]
    fn test_tier_a_assignment_for_750_salaried_experienced() {
        let config = ZdAiConfig::default();
        let mut app = base_app();
        app.total_experience_months = Some(24);
        let kyc = base_kyc();
        let mut bureau = base_bureau();
        bureau.cibil_score = Some(780);
        let extracted = base_extracted();
        let emp = EmployerVerifInput { status: "verified".into() };

        let res = evaluate_rules(&config, &app, &kyc, &bureau, &extracted, &[], Some(&emp), None, None, false, 1);
        assert_eq!(res.decision, AssessmentDecision::AutoApprove);
        assert_eq!(res.tier, Some(TenantTier::A));
    }

    #[test]
    fn test_action_required_on_missing_employer_verification() {
        let config = ZdAiConfig::default();
        let app = base_app();
        let kyc = base_kyc();
        let bureau = base_bureau();
        let extracted = base_extracted();

        // No employer verification provided
        let res = evaluate_rules(&config, &app, &kyc, &bureau, &extracted, &[], None, None, None, false, 1);
        assert_eq!(res.decision, AssessmentDecision::ActionRequired);
        assert!(res.pending_actions.contains(&"employer_email".to_string()));
    }

    #[test]
    fn test_manual_review_on_tier_c() {
        let config = ZdAiConfig::default();
        let mut app = base_app();
        app.monthly_rent = 12000.0;
        let kyc = base_kyc();
        let mut bureau = base_bureau();
        bureau.cibil_score = Some(670); // Tier C
        let extracted = base_extracted();
        let emp = EmployerVerifInput { status: "verified".into() };
        let co = CoApplicantInput {
            status: "accepted".into(),
            income: 30000.0,
            cibil_score: Some(720),
            emi_total: 0.0,
            invited_at: Utc::now(),
        };
        let lref = LandlordRefInput { status: "verified".into() };

        let res = evaluate_rules(&config, &app, &kyc, &bureau, &extracted, &[], Some(&emp), Some(&co), Some(&lref), false, 1);
        assert_eq!(res.decision, AssessmentDecision::ManualReview);
        assert_eq!(res.tier, Some(TenantTier::C));
        assert!(res.reason_codes.contains(&TIER_C_REVIEW.to_string()));
    }

    #[test]
    fn test_ai_flags_lead_to_manual_review_not_autoreject() {
        let config = ZdAiConfig::default();
        let app = base_app();
        let kyc = base_kyc();
        let bureau = base_bureau();
        let extracted = base_extracted();
        let emp = EmployerVerifInput { status: "verified".into() };

        let res = evaluate_rules(
            &config,
            &app,
            &kyc,
            &bureau,
            &extracted,
            &[DOC_TAMPER_SUSPECTED.to_string()],
            Some(&emp),
            None,
            None,
            false,
            1,
        );

        assert_eq!(res.decision, AssessmentDecision::ManualReview);
        assert!(res.reason_codes.contains(&DOC_TAMPER_SUSPECTED.to_string()));
    }
}
