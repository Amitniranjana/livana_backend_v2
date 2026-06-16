use axum::{
    routing::{get, patch, post, delete},
    Router, middleware,
};
use crate::{
    app_state::AppState,
    handlers::admin_users::{
        bulk_action_users, force_delete_user, get_user_detail, get_users, reinstate_user,
        suspend_user, update_user,
    },
    utils::admin_auth_guard::admin_auth_guard,
};

pub fn admin_users_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/admin/users", get(get_users))
        .route("/api/admin/users/bulk-action", post(bulk_action_users))
        .route("/api/admin/users/{id}", get(get_user_detail))
        .route("/api/admin/users/{id}", patch(update_user))
        .route("/api/admin/users/{id}/suspend", post(suspend_user))
        .route("/api/admin/users/{id}/reinstate", post(reinstate_user))
        .route("/api/admin/users/{id}/force", delete(force_delete_user))
        .route_layer(middleware::from_fn_with_state(state, admin_auth_guard))
}
