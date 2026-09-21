use axum::{
    routing::{get, post},
    Router, middleware,
};

use crate::{
    app_state::AppState,
    handlers::nbfc::{get_nbfc_zero_deposit_by_id, get_nbfc_zero_deposits, nbfc_login},
    utils::nbfc_auth_guard::nbfc_auth_guard,
};

pub fn nbfc_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(nbfc_login))
        .nest(
            "/zero-deposits",
            Router::new()
                .route("/", get(get_nbfc_zero_deposits))
                .route("/:id", get(get_nbfc_zero_deposit_by_id))
                .route_layer(middleware::from_fn_with_state(state, nbfc_auth_guard))
        )
}
