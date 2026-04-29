use crate::app_state::AppState;
use crate::handlers::carecrew::{
    cancel_booking, create_booking, edit_provider_profile, get_booking_details,
    get_featured_providers, get_provider, get_provider_bookings, get_provider_bookings_v2,
    get_service, get_user_bookings, list_services, search_providers, update_booking_status,
};
use axum::{
    Router,
    routing::{get, post, put},
};

pub fn carecrew_routes() -> Router<AppState> {
    Router::new()
        // Service endpoints (public)
        .route("/api/v1/carecrew/services", get(list_services))
        .route("/api/v1/carecrew/services/{id}", get(get_service))
        // Provider endpoints (public)
        .route("/api/v1/carecrew/providers", get(search_providers))
        .route(
            "/api/v1/carecrew/providers/featured",
            get(get_featured_providers),
        )
        .route(
            "/api/v1/carecrew/providers/{id}",
            get(get_provider).put(edit_provider_profile),
        )
        // Booking endpoints (authenticated)
        .route("/api/v1/carecrew/bookings", post(create_booking))
        .route(
            "/api/v1/carecrew/bookings/{id}/status",
            put(update_booking_status),
        )
        .route(
            "/api/v1/carecrew/providers/{id}/bookings",
            get(get_provider_bookings),
        )
}

pub fn bookings_routes() -> Router<AppState> {
    Router::new()
        .route("/api/bookings", get(get_user_bookings))
        .route("/api/bookings/provider", get(get_provider_bookings_v2))
        .route("/api/bookings/{id}", get(get_booking_details))
        .route("/api/bookings/{id}/status", put(update_booking_status))
        .route("/api/bookings/{id}/cancel", put(cancel_booking))
}
