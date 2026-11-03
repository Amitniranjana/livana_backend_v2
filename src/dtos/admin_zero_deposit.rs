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
