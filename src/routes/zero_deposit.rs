use axum::{routing::{get, post}, Router};
use crate::app_state::AppState;
use crate::handlers::zero_deposit::{
    apply_zero_deposit, credit_check_consent, get_zero_deposit,
    get_my_zero_deposits, get_status, charge_fee
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/me", get(get_my_zero_deposits))
        .route("/apply", post(apply_zero_deposit))
        .route("/credit-check/consent", post(credit_check_consent))
        .route("/{zero_deposit_id}", get(get_zero_deposit))
        .route("/{application_id}/status", get(get_status))
        .route("/{application_id}/fee/charge", post(charge_fee))
}
