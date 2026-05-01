use crate::app_state::AppState;
use crate::handlers::unified_listing::{
    create_listing, get_listing_by_id, list_listings, upload_listing_images_v2,
};
use axum::{
    Router,
    routing::{get, post},
};

pub fn unified_listing_routes() -> Router<AppState> {
    Router::new()
        // Create + List (paginated with filters)
        .route("/api/listings", post(create_listing).get(list_listings))
        // Get single listing with images
        .route("/api/listings/{id}", get(get_listing_by_id))
        // Image upload (S3 multipart) — uses /v2/ path to avoid conflict with legacy upload endpoint
        .route(
            "/api/listings/v2/upload/images",
            post(upload_listing_images_v2)
                .layer(axum::extract::DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
}
