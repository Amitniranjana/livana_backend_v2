use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::Type, PartialEq)]
#[sqlx(type_name = "zero_deposit_status", rename_all = "snake_case")]
pub enum ZeroDepositStatus {
    Applied,
    PendingReview,
    Blocked,
    Approved,
    Rejected,
    FeePending,
    Disbursed,
    // v2 statuses (Issue 107)
    AiReview,
    ActionRequired,
    AiManualReview,
    TokensIssued,
    RedemptionInProgress,
    Released,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::Type, PartialEq)]
#[sqlx(type_name = "fee_transaction_status", rename_all = "snake_case")]
pub enum FeeTransactionStatus {
    Pending,
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct FeeTransaction {
    pub id: Uuid,
    pub zero_deposit_id: Uuid,
    pub upi_vpa: String,
    pub amount: f64,
    pub status: FeeTransactionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ZeroDepositApplication {
    pub id: Uuid,
    pub user_id: Uuid,
    pub property_id: Option<Uuid>,
    pub kyc_id: Uuid,
    
    pub monthly_rent: f64,
    #[sqlx(default)]
    pub requested_deposit_amount: f64,
    pub monthly_income: f64,
    
    pub itr_document_url: Option<String>,
    pub bank_statement_url: Option<String>,
    pub consent_given: bool,
    pub status: ZeroDepositStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // v2 fields (Issue 107)
    #[sqlx(default)]
    pub tenancy_months: Option<i32>,
    #[sqlx(default)]
    pub employment_type: Option<String>,
    #[sqlx(default)]
    pub employer_name: Option<String>,
    #[sqlx(default)]
    pub current_job_start_date: Option<NaiveDate>,
    #[sqlx(default)]
    pub total_experience_months: Option<i32>,
    #[sqlx(default)]
    pub business_start_date: Option<NaiveDate>,
    #[sqlx(default)]
    pub variable_pay_monthly: Option<f64>,
    #[sqlx(default)]
    pub is_repeat_renter: bool,
    #[sqlx(default)]
    pub occupants: serde_json::Value,
    #[sqlx(default)]
    pub document_urls: serde_json::Value,
    #[sqlx(default)]
    pub deposit_amount: Option<f64>,
    #[sqlx(default)]
    pub fee_amount: Option<f64>,
    #[sqlx(default)]
    pub pending_actions: Vec<String>,
    #[sqlx(default)]
    pub bureau_summary: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ApplyZeroDepositRequest {
    pub property_id: Option<String>,
    pub kyc_id: String,
    pub monthly_rent: f64,
    pub requested_deposit_amount: Option<f64>,
    pub deposit_amount: Option<f64>,
    pub monthly_income: f64,
    pub itr_document_url: Option<String>,
    pub bank_statement_url: Option<String>,
    pub tenancy_months: Option<i32>,
    pub employment_type: Option<String>,
    pub employer_name: Option<String>,
    pub current_job_start_date: Option<NaiveDate>,
    pub total_experience_months: Option<i32>,
    pub business_start_date: Option<NaiveDate>,
    pub variable_pay_monthly: Option<f64>,
    pub is_repeat_renter: Option<bool>,
    pub occupants: Option<serde_json::Value>,
    pub document_urls: Option<serde_json::Value>,
    /// Issue 96/97: Opaque device fingerprint string from the client SDK.
    pub device_fingerprint: Option<String>,
    /// Issue 96: SHA-256 hex of "IFSC:account_number" (lowercased).
    pub bank_account_hash: Option<String>,
    /// Issue 96: Stated employer name, alias employer_name or company_name
    #[serde(alias = "stated_employer", alias = "company_name")]
    pub stated_employer: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreditCheckConsentRequest {
    pub zero_deposit_id: String,
    pub consent_given: bool,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct ZeroDepositStatusResponse {
    pub id: Uuid,
    pub status: ZeroDepositStatus,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChargeFeeRequest {
    pub upi_vpa: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::Type, PartialEq)]
#[sqlx(type_name = "mandate_type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MandateType {
    #[sqlx(rename = "UPI_AUTOPAY")]
    UpiAutopay,
    #[sqlx(rename = "NACH")]
    Nach,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::Type, PartialEq)]
#[sqlx(type_name = "autopay_status", rename_all = "snake_case")]
pub enum AutopayStatus {
    Pending,
    Active,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::Type, PartialEq)]
#[sqlx(type_name = "repayment_status", rename_all = "snake_case")]
pub enum RepaymentStatus {
    Pending,
    Paid,
    Overdue,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::Type, PartialEq)]
#[sqlx(type_name = "ledger_transaction_type", rename_all = "snake_case")]
pub enum LedgerTransactionType {
    Disbursement,
    Repayment,
    Fee,
    Refund,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct AutopayMandate {
    pub id: Uuid,
    pub zero_deposit_id: Uuid,
    pub mandate_type: MandateType,
    pub upi_vpa: Option<String>,
    pub bank_account_number: Option<String>,
    pub ifsc: Option<String>,
    pub status: AutopayStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct RepaymentSchedule {
    pub id: Uuid,
    pub zero_deposit_id: Uuid,
    pub installment_number: i32,
    pub due_date: NaiveDate,
    pub amount_due: f64,
    pub status: RepaymentStatus,
    pub partner_repayment_id: Option<String>,
    pub amount_paid: f64,
    pub paid_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct LedgerEntry {
    pub id: Uuid,
    pub zero_deposit_id: Uuid,
    pub transaction_type: LedgerTransactionType,
    pub amount: f64,
    pub reference_id: Option<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetupAutopayRequest {
    pub mandate_type: MandateType,
    pub upi_vpa: Option<String>,
    pub bank_account_number: Option<String>,
    pub ifsc: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RepaymentWebhookRequest {
    pub zero_deposit_id: Uuid,
    pub repayment_id: Uuid,
    pub due_date: Option<NaiveDate>,
    pub amount_paid: f64,
    pub status: String,
    pub paid_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FeeWebhookRequest {
    pub zero_deposit_id: Uuid,
    pub upi_txn_ref: String,
    pub status: String,
}

// =============================================================================
// ZD v2 Models (Issue 107)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "zd_tenant_tier", rename_all = "UPPERCASE")]
pub enum ZdTenantTier {
    A,
    B,
    C,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ZdTenantTierRecord {
    pub id: Uuid,
    pub zero_deposit_id: Uuid,
    pub tier: ZdTenantTier,
    pub cibil_score: Option<i32>,
    pub income_multiple: Option<f64>,
    pub rent_cap_pct: Option<f64>,
    pub rules_version: Option<String>,
    pub assigned_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "zd_token_allocation_status", rename_all = "snake_case")]
pub enum ZdTokenAllocationStatus {
    Active,
    Expired,
    Revoked,
    Exhausted,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ZdTokenAllocation {
    pub id: Uuid,
    pub zero_deposit_id: Uuid,
    #[sqlx(default)]
    pub user_id: Option<Uuid>,
    #[sqlx(default)]
    pub property_id: Option<Uuid>,
    #[sqlx(default)]
    pub assessment_id: Option<Uuid>,
    #[sqlx(default)]
    pub source: Option<String>,
    pub issued: f64,
    pub available: f64,
    pub locked: f64,
    pub redeemed: f64,
    pub status: ZdTokenAllocationStatus,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    #[sqlx(default)]
    pub issued_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "zd_token_operation", rename_all = "snake_case")]
pub enum ZdTokenOperation {
    Issue,
    Lock,
    Unlock,
    Redeem,
    Expire,
    Revoke,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ZdTokenTransaction {
    pub id: Uuid,
    pub allocation_id: Uuid,
    pub zero_deposit_id: Uuid,
    pub operation: ZdTokenOperation,
    pub amount: f64,
    #[sqlx(default)]
    pub available_after: Option<f64>,
    #[sqlx(default)]
    pub locked_after: Option<f64>,
    #[sqlx(default)]
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    #[sqlx(default)]
    pub actor_type: Option<String>,
    #[sqlx(default)]
    pub actor_id: Option<Uuid>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "zd_redemption_status", rename_all = "snake_case")]
pub enum ZdRedemptionStatus {
    Draft,
    Submitted,
    DocCheckPending,
    DocCheckDone,
    AdminReview,
    Approved,
    Rejected,
    Cancelled,
    Completed,
    // V2 formal statuses
    DocsPending,
    UnderReview,
    ChangesRequested,
    Released,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ZdRedemption {
    pub id: Uuid,
    pub zero_deposit_id: Uuid,
    pub allocation_id: Uuid,
    #[sqlx(default)]
    pub user_id: Option<Uuid>,
    #[sqlx(default)]
    pub chat_id: Option<Uuid>,
    pub tokens_requested: f64,
    pub tokens_locked: Option<f64>,
    #[sqlx(default)]
    pub tokens_approved: Option<f64>,
    pub status: ZdRedemptionStatus,
    #[sqlx(default)]
    pub payee_snapshot: Option<serde_json::Value>,
    pub property_address: Option<String>,
    pub monthly_rent: Option<f64>,
    pub landlord_name: Option<String>,
    pub landlord_phone: Option<String>,
    pub landlord_bank_account: Option<String>,
    pub landlord_ifsc: Option<String>,
    pub doc_check_id: Option<String>,
    pub doc_check_flags: serde_json::Value,
    pub admin_remarks: Option<String>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    #[sqlx(default)]
    pub released_by: Option<Uuid>,
    #[sqlx(default)]
    pub released_at: Option<DateTime<Utc>>,
    #[sqlx(default)]
    pub released_amount: Option<f64>,
    #[sqlx(default)]
    pub payout_mode: Option<String>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub utr: Option<String>,
    #[sqlx(default)]
    pub reject_reason: Option<String>,
    #[sqlx(default)]
    pub cancel_reason: Option<String>,
    #[sqlx(default)]
    pub idempotency_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ZdRedemptionDocument {
    pub id: Uuid,
    pub redemption_id: Uuid,
    pub doc_type: String,
    pub url: String,
    pub verified: bool,
    pub admin_notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ZdCoApplicant {
    pub id: Uuid,
    pub zero_deposit_id: Uuid,
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub relationship: Option<String>,
    pub monthly_income: Option<f64>,
    pub bureau_summary: serde_json::Value,
    pub consent_given: bool,
    pub consent_at: Option<DateTime<Utc>>,
    pub invite_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ZdEmployerVerification {
    pub id: Uuid,
    pub zero_deposit_id: Uuid,
    pub employer_name: Option<String>,
    pub verified_name: Option<String>,
    pub verification_source: Option<String>,
    pub is_verified: bool,
    pub raw_response: serde_json::Value,
    pub verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ZdLandlordReference {
    pub id: Uuid,
    pub zero_deposit_id: Uuid,
    pub landlord_name: Option<String>,
    pub landlord_phone: Option<String>,
    pub reference_status: String,
    pub contacted_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ZdAiConfig {
    pub id: Uuid,
    pub salaried_income_multiple: f64,
    pub self_employed_income_multiple: f64,
    pub business_income_multiple: f64,
    pub obligations_cap_pct: f64,
    pub rent_cap_pct: f64,
    pub tier_a_min_cibil: i32,
    pub tier_b_min_cibil: i32,
    pub tier_c_min_cibil: i32,
    pub tier_a_rent_cap_pct: f64,
    pub tier_b_rent_cap_pct: f64,
    pub tier_c_rent_cap_pct: f64,
    pub max_dpd_12m: i32,
    pub max_dpd_24m: i32,
    pub max_enquiries_3m: i32,
    pub max_bounces_6m: i32,
    pub ntc_alt_scoring_enabled: bool,
    pub token_validity_days: i32,
    pub ai_service_base_url: String,
    pub ai_service_timeout_secs: i32,
    pub ai_hmac_secret: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ZeroDepositAiAssessment {
    pub id: Uuid,
    pub zero_deposit_id: Uuid,
    pub assessment_id: String,
    pub status: String,
    pub recommendation: Option<String>,
    pub rules_version: Option<String>,
    pub extracted: serde_json::Value,
    pub ai_flags: serde_json::Value,
    pub admin_summary: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
