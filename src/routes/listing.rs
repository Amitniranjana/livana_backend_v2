use crate::app_state::AppState;
use crate::handlers::listing::{
    create_property, delete_property, get_broker_properties, get_properties, get_property_by_id,
    get_saved_properties, like_property, report_property, save_property, search_properties,
    unlike_property, unsave_property, update_property,
};
use axum::{Router, routing::{delete, get, post, put}};

pub fn listing_routes() -> Router<AppState> {
    Router::new()
        // Collection
        .route("/api/properties", get(get_properties))
        .route("/api/properties", post(create_property))
        // Broker's own listings
        .route("/api/properties/broker", get(get_broker_properties))
        // Search
        .route("/api/properties/search", get(search_properties))
        // Saved properties list
        .route("/api/properties/saved", get(get_saved_properties))
        // Single property CRUD
        .route("/api/properties/{id}", get(get_property_by_id))
        .route("/api/properties/{id}", put(update_property))
        .route("/api/properties/{id}", delete(delete_property))
        // Like / Unlike
        .route("/api/properties/{id}/like", post(like_property))
        .route("/api/properties/{id}/like", delete(unlike_property))
        // Save / Unsave
        .route("/api/properties/{id}/save", post(save_property))
        .route("/api/properties/{id}/save", delete(unsave_property))
        // Report
        .route("/api/properties/{id}/report", post(report_property))
        // Listing Images Upload (New API)
        .route(
            "/api/listings/upload/images",
            post(crate::handlers::listing_image::upload_listing_images)
                .layer(axum::extract::DefaultBodyLimit::disable()),
        )
}
