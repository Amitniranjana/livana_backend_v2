use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ZeroDepositQuery {
    pub status: Option<String>,
    pub user_role: Option<String>,
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
pub struct ZeroDepositAssignNbfcReq {
    pub nbfc_id: Uuid,
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
pub struct NbfcPartnerReq {
    pub name: String,
    pub entity_type: String,
    pub revenue_share_pct: f64,
    pub fldg_buffer_pct: f64,
}

#[derive(Debug, Deserialize)]
pub struct NbfcPartnerUpdateReq {
    pub status: crate::models::nbfc::NbfcStatus,
}
