use axum::{
    middleware,
    routing::get,
    Router,
};
use crate::{
    app_state::AppState,
    handlers::admin_logs::{get_admin_logs, get_admin_logs_by_target},
    utils::admin_auth_guard::admin_auth_guard,
};

pub fn admin_logs_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/admin/logs", get(get_admin_logs))
        .route("/api/admin/logs/{target_type}/{target_id}", get(get_admin_logs_by_target))
        .route_layer(middleware::from_fn_with_state(state, admin_auth_guard))
}
