use axum::{Router, routing::{get, post, put, delete}};
use crate::app_state::AppState;
use crate::handlers::listing::{
    get_properties, create_property, get_broker_properties, search_properties,
    get_property_by_id, update_property, delete_property,
    like_property, unlike_property, save_property, unsave_property,
    get_saved_properties, report_property,
};

pub fn health_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(crate::handlers::health::get_health))
}

pub fn auth_routes() -> Router<AppState> {
    use crate::handlers::auth::{signup, signin, signout, send_forgot_password_link, reset_password};
    Router::new()
        .route("/api/auth/signup", post(signup))
        .route("/api/auth/signin", post(signin))
        .route("/api/auth/signout", post(signout))
        .route("/api/auth/send-forgot-password-link", post(send_forgot_password_link))
        .route("/api/auth/reset-password", post(reset_password))
        .route("/auth/google", post(crate::handlers::google_auth::google_signin))
}

pub fn user_routes() -> Router<AppState> {
    use crate::handlers::user::{get_profile, update_profile, upload_profile_image};
    Router::new()
        .route("/api/user/profile", get(get_profile))
        .route("/api/user/profile", put(update_profile))
        .route("/api/user/profile/upload-image", post(upload_profile_image))
}


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
}


pub fn broker_routes() -> Router<AppState> {
    use crate::handlers::broker::{onboarding, get_profile};
    Router::new()
        .route("/api/broker/onboarding", post(onboarding))
        .route("/api/broker/profile", get(get_profile))
}

pub fn associate_routes() -> Router<AppState> {
    use crate::handlers::associate::{
        register_associate, upload_kyc_documents, get_associate_profile, get_associate_types,
    };
    Router::new()
        .route("/api/v1/associates/register", post(register_associate))
        .route("/api/v1/associates/{id}/kyc", post(upload_kyc_documents))
        .route("/api/v1/associates/me", get(get_associate_profile))
        .route("/api/v1/associate-types", get(get_associate_types))
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
