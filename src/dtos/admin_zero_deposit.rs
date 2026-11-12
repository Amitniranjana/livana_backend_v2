use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ZeroDepositQuery {
    pub status: Option<String>,
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
