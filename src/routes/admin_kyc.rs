use axum::{
    middleware,
    routing::{get, patch},
    Router,
};
use crate::{
    app_state::AppState,
    handlers::admin_kyc::{
        approve_kyc_submission, get_kyc_submission_detail, get_kyc_submissions,
        reject_kyc_submission,
    },
    utils::admin_auth_guard::admin_auth_guard,
};

pub fn admin_kyc_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/admin/kyc", get(get_kyc_submissions))
        .route("/api/admin/kyc/{kyc_id}", get(get_kyc_submission_detail))
        .route("/api/admin/kyc/{kyc_id}/approve", patch(approve_kyc_submission))
        .route("/api/admin/kyc/{kyc_id}/reject", patch(reject_kyc_submission))
        .route_layer(middleware::from_fn_with_state(state, admin_auth_guard))
}
