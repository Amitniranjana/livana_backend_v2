use axum::{
    middleware,
    routing::{get, post, delete},
    Router,
};
use crate::{
    app_state::AppState,
    handlers::admin_reports::{
        get_admin_report_detail, get_admin_reports, update_report_status,
        delete_report, execute_report_action,
    },
    utils::admin_auth_guard::admin_auth_guard,
};

pub fn admin_reports_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/admin/reports", get(get_admin_reports))
        .route("/api/admin/reports/{id}", get(get_admin_report_detail).patch(update_report_status))
        .route("/api/admin/reports/{id}/force", delete(delete_report))
        .route("/api/admin/reports/{id}/action", post(execute_report_action))
        .route_layer(middleware::from_fn_with_state(state, admin_auth_guard))
}
