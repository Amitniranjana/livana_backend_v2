use axum::{
    middleware,
    routing::{get, patch},
    Router,
};
use crate::{
    app_state::AppState,
    handlers::admin_reports::{get_admin_report_detail, get_admin_reports, update_report_status},
    utils::admin_auth_guard::admin_auth_guard,
};

pub fn admin_reports_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/admin/reports", get(get_admin_reports))
        .route("/api/admin/reports/{id}", get(get_admin_report_detail))
        .route("/api/admin/reports/{id}/status", patch(update_report_status))
        .route_layer(middleware::from_fn_with_state(state, admin_auth_guard))
}
