use axum::{Router, routing::{get, post, put, delete}};
use crate::app_state::AppState;
use crate::handlers::{
    health::get_health,
    auth::{signup, signin, signout, send_forgot_password_link, reset_password},
    user::{get_profile, update_profile, upload_profile_image},
    listing::{get_listings, create_listing, get_listing_by_id, update_listing, delete_listing, like_listing, save_listing},
};

pub fn health_routes()-> Router<AppState>{
    Router::new()
        .route("/health", get(get_health))

}
pub fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/api/auth/signup", post(signup))
        .route("/api/auth/signin", post(signin))
        .route("/api/auth/signout", post(signout))
        .route("/api/auth/send-forgot-password-link", post(send_forgot_password_link))
        .route("/api/auth/reset-password", post(reset_password))
}

pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/api/user/profile", get(get_profile))
        .route("/api/user/profile", put(update_profile))
        .route("/api/user/profile/upload-image", post(upload_profile_image))
}

pub fn listing_routes() -> Router<AppState> {
    Router::new()
        .route("/api/listings", get(get_listings))
        .route("/api/listings", post(create_listing))
        .route("/api/listings/{id}", get(get_listing_by_id))
        .route("/api/listings/{id}", put(update_listing))
        .route("/api/listings/{id}", delete(delete_listing))
        .route("/api/listings/{id}/like", post(like_listing))
        .route("/api/listings/{id}/save", post(save_listing))
}

pub mod chat_routes;
pub use chat_routes::chat_routes;