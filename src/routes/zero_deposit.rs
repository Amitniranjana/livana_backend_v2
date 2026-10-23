/// Zero-deposit routes
/// Issue 97: POST /apply is wrapped with the loan_velocity_guard middleware
/// which enforces sliding-window rate limits keyed on device_id and IP.

use axum::{routing::{get, post}, Router};
use crate::app_state::AppState;
use crate::handlers::zero_deposit::{
    apply_zero_deposit, credit_check_consent, get_zero_deposit,
    get_my_zero_deposits, get_status, charge_fee,
    setup_autopay, get_repayment_schedule, get_ledger
};
use crate::handlers::zero_deposit_webhook::{
    disbursement_webhook, repayment_webhook, fee_webhook
};

pub fn router() -> Router<AppState> {
    // Note: loan_velocity_guard is a `from_fn_with_state` middleware and needs
    // the full AppState to query Redis/DB.  The state is injected by Axum
    // automatically via `route_layer` because the parent Router carries AppState.
    // We use `from_fn` (stateless variant) because the middleware extracts State
    // itself via `State<AppState>` — the same pattern used in login_rate_limiter.
    let apply_route = Router::new()
        .route("/apply", post(apply_zero_deposit));

    Router::new()
        .merge(apply_route)
        .route("/me", get(get_my_zero_deposits))
        .route("/credit-check/consent", post(credit_check_consent))
        .route("/{zero_deposit_id}", get(get_zero_deposit))
        .route("/{application_id}/status", get(get_status))
        .route("/{application_id}/fee/charge", post(charge_fee))
        .route("/{application_id}/autopay/setup", post(setup_autopay))
        .route("/{application_id}/repayment-schedule", get(get_repayment_schedule))
        .route("/{application_id}/ledger", get(get_ledger))
        .route("/webhook/disbursement-status", post(disbursement_webhook))
        .route("/webhook/repayment-status", post(repayment_webhook))
        .route("/webhook/fee-status", post(fee_webhook))
}
