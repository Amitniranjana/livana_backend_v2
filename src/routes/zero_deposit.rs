/// Zero-deposit routes
/// Issue 97: POST /apply is wrapped with the loan_velocity_guard middleware
/// which enforces sliding-window rate limits keyed on device_id and IP.

use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use crate::app_state::AppState;
use crate::handlers::zero_deposit::{
    apply_zero_deposit, credit_check_consent, get_zero_deposit,
    get_my_zero_deposits, get_status, get_assessment, charge_fee,
    setup_autopay, get_repayment_schedule, get_ledger
};
use crate::handlers::zero_deposit_webhook::{
    repayment_webhook, fee_webhook, esign_webhook
};
use crate::handlers::zd_actions_handler::{
    send_employer_otp, verify_employer_otp, invite_co_applicant,
    remove_co_applicant, get_my_co_applicant_invites, accept_co_applicant_invite,
    add_landlord_reference
};
use crate::handlers::zd_wallet_handler::{
    get_token_wallet, get_token_transactions
};
use crate::handlers::zd_redemption_handler::{
    cancel_redemption, get_esign_link, get_my_redemptions, get_redemption_detail,
    get_redemption_messages, open_redemption_chat, send_redemption_message,
    submit_redemption, upload_redemption_document,
};

pub fn router(state: AppState) -> Router<AppState> {
    let apply_route = Router::new()
        .route("/apply", post(apply_zero_deposit))
        .route_layer(middleware::from_fn_with_state(
            state,
            crate::utils::loan_velocity::loan_velocity_guard,
        ));

    Router::new()
        .merge(apply_route)
        .route("/me", get(get_my_zero_deposits))
        .route("/credit-check/consent", post(credit_check_consent))
        // Static endpoints (must precede /{zero_deposit_id})
        .route("/tokens/wallet", get(get_token_wallet))
        .route("/tokens/transactions", get(get_token_transactions))
        .route("/redemptions/me", get(get_my_redemptions))
        .route("/redemptions/{redemption_id}", get(get_redemption_detail))
        .route("/redemptions/{redemption_id}/messages", get(get_redemption_messages).post(send_redemption_message))
        .route("/redemptions/{redemption_id}/documents", post(upload_redemption_document))
        .route("/redemptions/{redemption_id}/submit", post(submit_redemption))
        .route("/redemptions/{redemption_id}/esign", post(get_esign_link))
        .route("/redemptions/{redemption_id}/cancel", post(cancel_redemption))
        .route("/co-applicant-invites/me", get(get_my_co_applicant_invites))
        .route("/co-applicant-invites/{co_applicant_id}/accept", post(accept_co_applicant_invite))
        .route("/webhook/repayment-status", post(repayment_webhook))
        .route("/webhook/fee-status", post(fee_webhook))
        .route("/webhook/esign-status", post(esign_webhook))
        // Per-application endpoints
        .route("/{zero_deposit_id}", get(get_zero_deposit))
        .route("/{zero_deposit_id}/redemptions", post(open_redemption_chat))
        .route("/{zero_deposit_id}/assessment", get(get_assessment))
        .route("/{zero_deposit_id}/status", get(get_status))
        .route("/{zero_deposit_id}/fee/charge", post(charge_fee))
        .route("/{zero_deposit_id}/autopay/setup", post(setup_autopay))
        .route("/{zero_deposit_id}/repayment-schedule", get(get_repayment_schedule))
        .route("/{zero_deposit_id}/ledger", get(get_ledger))
        .route("/{zero_deposit_id}/employer-verification/send-otp", post(send_employer_otp))
        .route("/{zero_deposit_id}/employer-verification/verify", post(verify_employer_otp))
        .route("/{zero_deposit_id}/co-applicant", post(invite_co_applicant))
        .route("/{zero_deposit_id}/co-applicant/{co_applicant_id}", delete(remove_co_applicant))
        .route("/{zero_deposit_id}/landlord-reference", post(add_landlord_reference))
}
