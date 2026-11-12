use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ZeroDepositQuery {
    pub status: Option<String>,
    pub decision: Option<String>,
    pub tier: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ZeroDepositApproveReq {
    pub approved_deposit_amount: f64,
    pub remarks: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ZeroDepositRejectReq {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct ZeroDepositDefaultActionReq {
    pub remarks: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ZeroDepositRecoverReq {
    pub recovered_amount: f64,
    pub remarks: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AssessmentOverrideReq {
    pub decision: String, // "approve" | "reject"
    pub tier: Option<String>, // "A" | "B" | "C"
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct AssessmentRerunReq {
    pub remarks: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserTierUpdateReq {
    pub tier: String, // "A" | "B" | "C"
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ManualIssueReq {
    pub tier: String, // "A" | "B" | "C"
    pub remarks: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RevokeTokensReq {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct MonthlyReportQuery {
    pub year: Option<i32>,
    pub month: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct RedemptionApproveReq {
    pub tokens_approved: f64,
    pub payee_name: Option<String>,
    pub payee_account_number: Option<String>,
    pub payee_ifsc: Option<String>,
    pub remarks: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RedemptionReleaseReq {
    pub utr: String,
    pub released_amount: f64,
    pub payout_mode: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RedemptionRejectReq {
    pub reason: String,
}

