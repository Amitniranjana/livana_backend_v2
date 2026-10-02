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
    
    // Using f64 for NUMERIC mapping based on existing models
    pub monthly_rent: f64,
    pub requested_deposit_amount: f64,
    pub monthly_income: f64,
    
    pub itr_document_url: Option<String>,
    pub bank_statement_url: Option<String>,
    pub consent_given: bool,
    pub status: ZeroDepositStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ApplyZeroDepositRequest {
    pub property_id: Option<String>, // Passed as string and parsed into UUID
    pub kyc_id: String,
    pub monthly_rent: f64,
    pub requested_deposit_amount: f64,
    pub monthly_income: f64,
    pub itr_document_url: Option<String>,
    pub bank_statement_url: Option<String>,
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
pub struct DisbursementWebhookRequest {
    pub zero_deposit_id: Uuid,
    pub status: String,
    pub amount: f64,
    pub disbursed_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RepaymentWebhookRequest {
    pub zero_deposit_id: Uuid,
    pub repayment_id: Uuid, // or String depending on what the partner sends, let's use String as partner_repayment_id
    pub due_date: Option<NaiveDate>,
    pub amount_paid: f64,
    pub status: String, // e.g., "paid", "missed"
    pub paid_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FeeWebhookRequest {
    pub zero_deposit_id: Uuid,
    pub upi_txn_ref: String,
    pub status: String, // e.g., "success", "failed"
}
