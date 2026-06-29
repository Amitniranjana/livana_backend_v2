use axum::{
    routing::{get, patch, post, delete},
    Router, middleware,
};
use crate::{
    app_state::AppState,
    handlers::admin_properties::{
        bulk_action_properties, force_delete_property, get_properties, get_property_detail, update_property,
    },
    utils::admin_auth_guard::admin_auth_guard,
};

pub fn admin_properties_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/admin/properties", get(get_properties))
        .route("/api/admin/properties/bulk-action", post(bulk_action_properties))
        .route("/api/admin/properties/{id}", get(get_property_detail))
        .route("/api/admin/properties/{id}", patch(update_property))
        .route("/api/admin/properties/{id}/force", delete(force_delete_property))
        .route_layer(middleware::from_fn_with_state(state, admin_auth_guard))
}
