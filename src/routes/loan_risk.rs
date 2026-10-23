/// Loan risk + fraud detection routes
/// Issues 93 / 94 / 95 / 96 / 97
///
/// Admin routes (require admin JWT):
///   GET  /api/admin/loans/{loan_id}/risk-score         — Issue 93
///   GET  /api/admin/loans/{loan_id}/decision-reasons   — Issue 95
///   POST /api/admin/loans/{loan_id}/approve            — Issue 95
///   POST /api/admin/loans/{loan_id}/reject             — Issue 95
///   GET  /api/admin/loans/fraud-signals                — Issue 96
///   GET  /api/admin/loans/velocity-config              — Issue 97 (read-only config)
///
/// User/system route:
///   POST /api/v1/loans/{loan_id}/bank-statement/analyze  — Issue 94

use axum::{
    middleware,
    routing::{get, post, patch},
    Router,
};
use crate::{
    app_state::AppState,
    handlers::loan_risk::{
        get_loan_risk_score,
        analyze_bank_statement,
        approve_loan_with_reasons,
        reject_loan_with_reasons,
        get_loan_decision_reasons,
    },
    handlers::fraud_admin::{
        get_fraud_signals,
        get_velocity_config,
        trigger_batch_fraud_scan,
    },
    utils::admin_auth_guard::admin_auth_guard,
};

pub fn loan_risk_routes(state: AppState) -> Router<AppState> {
    // Admin-protected routes
    let admin_routes = Router::new()
        .route("/api/admin/loans/{loan_id}/risk-score",        get(get_loan_risk_score))
        .route("/api/admin/loans/{loan_id}/decision-reasons",  get(get_loan_decision_reasons))
        .route("/api/admin/loans/{loan_id}/approve",           patch(approve_loan_with_reasons))
        .route("/api/admin/loans/{loan_id}/reject",            patch(reject_loan_with_reasons))
        // Issue 96: fraud-signals admin dashboard view & batch scan
        .route("/api/admin/loans/fraud-signals",               get(get_fraud_signals))
        .route("/api/admin/loans/fraud-signals/scan",          post(trigger_batch_fraud_scan))
        // Issue 97: velocity-config read-only (lets admin UI display current limits)
        .route("/api/admin/loans/velocity-config",             get(get_velocity_config))
        .route_layer(middleware::from_fn_with_state(state.clone(), admin_auth_guard));

    // System / user-facing routes (no admin auth — called after upload completes)
    let system_routes = Router::new()
        .route(
            "/api/v1/loans/{loan_id}/bank-statement/analyze",
            post(analyze_bank_statement),
        );

    // Issue 97: POST /api/v1/loans/apply wrapped with the velocity check middleware
    let loan_apply_route = Router::new()
        .route(
            "/api/v1/loans/apply",
            post(crate::handlers::zero_deposit::apply_zero_deposit),
        )
        .route_layer(middleware::from_fn_with_state(
            state,
            crate::utils::loan_velocity::loan_velocity_guard,
        ));

    Router::new()
        .merge(admin_routes)
        .merge(system_routes)
        .merge(loan_apply_route)
}
