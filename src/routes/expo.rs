use crate::app_state::AppState;
use crate::handlers::expo::{
    create_expo, edit_expo, get_all_expos, get_expo_details, get_expo_participants,
    register_for_expo,
};
use axum::{Router, routing::{get, post}};

pub fn expo_routes() -> Router<AppState> {
    Router::new()
        .route("/api/expo", post(create_expo))
        .route("/api/expo", get(get_all_expos))
        .route("/api/expo/{expo_id}", get(get_expo_details).put(edit_expo))
        .route("/api/expo/{expo_id}/register", post(register_for_expo))
        .route(
            "/api/expo/{expo_id}/participants",
            get(get_expo_participants),
        )
}
