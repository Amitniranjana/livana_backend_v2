use chrono::{DateTime, Utc};
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
