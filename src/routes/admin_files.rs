use axum::{middleware, routing::get, Router};

use crate::{
    app_state::AppState,
    handlers::admin_files::view_file,
    utils::admin_auth_guard::admin_auth_guard,
};

pub fn admin_files_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/admin/files/view", get(view_file))
        .route_layer(middleware::from_fn_with_state(state, admin_auth_guard))
}

