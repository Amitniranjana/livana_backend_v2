use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::Type, PartialEq)]
#[sqlx(type_name = "loan_status", rename_all = "snake_case")]
pub enum LoanStatus {
    Applied,
    PendingReview,
    Blocked,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct LoanApplication {
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
    pub status: LoanStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ApplyLoanRequest {
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
    pub loan_id: String,
    pub consent_given: bool,
}
