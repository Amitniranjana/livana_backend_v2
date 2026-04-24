use crate::app_state::AppState;
use crate::handlers::broker::{get_profile, onboarding};
use axum::{Router, routing::{get, post}};

pub fn broker_routes() -> Router<AppState> {
    Router::new()
        .route("/api/broker/onboarding", post(onboarding))
        .route("/api/broker/profile", get(get_profile))
}
