use crate::app_state::AppState;
use crate::handlers::service_listing::{add_service, edit_service, get_all_services};
use axum::{
    Router,
    routing::{get, post, put},
};

pub fn service_listing_routes() -> Router<AppState> {
    Router::new()
        .route("/api/services", post(add_service).get(get_all_services))
        .route(
            "/api/services/providers",
            get(crate::handlers::service_listing::filter_providers),
        )
        .route("/api/services/{service_id}", put(edit_service))
}
