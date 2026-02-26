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
        // ── Google OAuth ─────────────────────────────────────────────────────
        .route("/auth/google", post(crate::handlers::google_auth::google_signin))
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

pub fn broker_routes() -> Router<AppState> {
    use crate::handlers::broker::{onboarding, get_profile};
    Router::new()
        .route("/api/broker/onboarding", post(onboarding))
        .route("/api/broker/profile", get(get_profile))
}

/// Property Search + Filters (Steps 1 & 2)
pub fn property_search_routes() -> Router<AppState> {
    use crate::handlers::property_search::{
        search_properties_handler,
        get_filters_handler,
    };
    Router::new()
        .route("/api/v1/properties/search",  get(search_properties_handler))
        .route("/api/v1/properties/filters", get(get_filters_handler))
}

/// Autocomplete / Suggestions (Step 3)
pub fn suggestions_routes() -> Router<AppState> {
    use crate::handlers::property_search::get_suggestions_handler;
    Router::new()
        .route("/api/v1/search/suggestions", get(get_suggestions_handler))
}

/// CareCrew Module (Step 4)
pub fn carecrew_routes() -> Router<AppState> {
    use crate::handlers::carecrew::{
        list_services,
        get_service,
        search_providers,
        get_featured_providers,
        get_provider,
        create_booking,
        update_booking_status,
        get_provider_bookings,
    };
    Router::new()
        // Service endpoints (public)
        .route("/api/v1/carecrew/services",                    get(list_services))
        .route("/api/v1/carecrew/services/{id}",               get(get_service))
        // Provider endpoints (public)
        .route("/api/v1/carecrew/providers",                   get(search_providers))
        .route("/api/v1/carecrew/providers/featured",          get(get_featured_providers))
        .route("/api/v1/carecrew/providers/{id}",              get(get_provider))
        // Booking endpoints (authenticated)
        .route("/api/v1/carecrew/bookings",                    post(create_booking))
        .route("/api/v1/carecrew/bookings/{id}/status",        put(update_booking_status))
        .route("/api/v1/carecrew/providers/{id}/bookings",     get(get_provider_bookings))
}

/// CareCrew Tickets (Support Module)
pub fn carecrew_ticket_routes() -> Router<AppState> {
    use axum::routing::patch;
    use crate::handlers::carecrew_tickets::{
        create_ticket_handler,
        list_tickets_handler,
        get_ticket_handler,
        update_ticket_handler,
        add_comment_handler,
    };
    Router::new()
        .route("/api/v1/carecrew/tickets",                     post(create_ticket_handler))
        .route("/api/v1/carecrew/tickets",                     get(list_tickets_handler))
        .route("/api/v1/carecrew/tickets/{ticketId}",          get(get_ticket_handler))
        .route("/api/v1/carecrew/tickets/{ticketId}",          patch(update_ticket_handler))
        .route("/api/v1/carecrew/tickets/{ticketId}/comments", post(add_comment_handler))
}

/// Recent Chats (JWT-protected)
pub fn recent_chats_routes() -> Router<AppState> {
    use crate::handlers::recent_chats::get_recent_chats;
    Router::new()
        .route("/api/v1/chats/recent", get(get_recent_chats))
}

pub mod chat_routes;
pub use chat_routes::chat_routes;

pub mod kyc;
pub use kyc::kyc_routes;
