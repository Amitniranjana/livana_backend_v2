use crate::app_state::AppState;
use crate::handlers::user::{get_profile, update_profile, upload_profile_image};
use axum::{Router, routing::{get, post, put}};

pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/api/user/profile", get(get_profile))
        .route("/api/user/profile", put(update_profile))
        .route("/api/user/profile/upload-image", post(upload_profile_image))
}
