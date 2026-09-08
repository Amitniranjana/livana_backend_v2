use axum::{routing::{get, post}, Router};
use crate::app_state::AppState;
use crate::handlers::loan::{apply_loan, credit_check_consent, get_loan};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/apply", post(apply_loan))
        .route("/credit-check/consent", post(credit_check_consent))
        .route("/{loan_id}", get(get_loan))
}
