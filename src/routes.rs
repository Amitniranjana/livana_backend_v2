use crate::app_state::AppState;
use crate::handlers::listing::{
    create_property, delete_property, get_broker_properties, get_properties, get_property_by_id,
    get_saved_properties, like_property, report_property, save_property, search_properties,
    unlike_property, unsave_property, update_property,
};
use axum::{
    Router,
    routing::{delete, get, post, put},
};

pub fn health_routes() -> Router<AppState> {
    Router::new().route("/health", get(crate::handlers::health::get_health))
}

pub fn auth_routes() -> Router<AppState> {
    use crate::handlers::auth::{
        change_password, resend_otp, reset_password, send_forgot_password_link, send_otp, signin,
        signout, signup, update_associate_type, verify_otp,
    };
    use axum::routing::patch;
    Router::new()
        .route("/api/auth/signup", post(signup))
        .route("/api/auth/signin", post(signin))
        .route("/api/auth/signout", post(signout))
        .route("/api/auth/send-otp", post(send_otp))
        .route("/api/auth/verify-otp", post(verify_otp))
        .route("/api/auth/resend-otp", post(resend_otp))
        .route("/api/auth/associate-type", patch(update_associate_type))
        .route(
            "/api/auth/send-forgot-password-link",
            post(send_forgot_password_link),
        )
        .route("/api/auth/reset-password", post(reset_password))
        .route("/api/auth/change-password", post(change_password))
        .route(
            "/auth/google",
            post(crate::handlers::google_auth::google_signin),
        )
}

pub fn admin_auth_routes() -> Router<AppState> {
    use crate::handlers::admin_auth::{admin_login, admin_logout, admin_me};
    use axum::routing::{get, post};
    Router::new()
        .route("/api/admin/auth/login", post(admin_login))
        .route("/api/admin/auth/logout", post(admin_logout))
        .route("/api/admin/auth/me", get(admin_me))
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
        // Listing Images Upload (New API)
        .route(
            "/api/listings/upload/images",
            post(crate::handlers::listing_image::upload_listing_images)
                .layer(axum::extract::DefaultBodyLimit::disable()),
        )
}

pub fn broker_routes() -> Router<AppState> {
    use crate::handlers::broker::{get_profile, onboarding};
    Router::new()
        .route("/api/broker/onboarding", post(onboarding))
        .route("/api/broker/profile", get(get_profile))
}

pub fn associate_routes() -> Router<AppState> {
    use crate::handlers::associate::{
        get_associate_profile, get_associate_types, register_associate, upload_kyc_documents,
    };
    Router::new()
        .route("/api/v1/associates/register", post(register_associate))
        .route("/api/v1/associates/{id}/kyc", post(upload_kyc_documents))
        .route("/api/v1/associates/me", get(get_associate_profile))
        .route("/api/v1/associate-types", get(get_associate_types))
}

pub fn career_routes() -> Router<AppState> {
    use crate::handlers::career::{get_career_detail, list_careers};
    Router::new()
        .route("/api/careers", get(list_careers))
        .route("/api/careers/jobs", get(list_careers)) // ALIAS
        .route("/api/careers/{job_id}", get(get_career_detail))
        .route("/api/careers/jobs/{job_id}", get(get_career_detail)) // ALIAS
}

pub fn careers_routes() -> Router<AppState> {
    use crate::handlers::careers::{
        apply_job, edit_job, get_applicants, get_job_detail, list_jobs, post_job,
        my_posted_jobs, update_application_status,
    };
    use axum::routing::patch;
    Router::new()
        .route("/api/v1/jobs", post(post_job).get(list_jobs))
        .route("/api/v1/jobs/mine", get(my_posted_jobs))
        .route("/api/v1/jobs/{job_id}", get(get_job_detail).put(edit_job))
        .route("/api/v1/jobs/{job_id}/apply", post(apply_job))
        .route("/api/v1/jobs/{job_id}/applicants", get(get_applicants))
        .route("/api/v1/jobs/{job_id}/applications/{application_id}/status", patch(update_application_status))
}

pub fn reviews_routes() -> Router<AppState> {
    use crate::handlers::reviews::{add_review, get_reviews};
    Router::new()
        .route("/api/v1/reviews", post(add_review))
        .route("/api/v1/associates/{id}/reviews", get(get_reviews))
}

/// Property Search + Filters (Steps 1 & 2)
pub fn property_search_routes() -> Router<AppState> {
    use crate::handlers::property_search::{get_filters_handler, search_properties_handler};
    Router::new()
        .route("/api/v1/properties/search", get(search_properties_handler))
        .route("/api/v1/properties/filters", get(get_filters_handler))
}

/// Autocomplete / Suggestions (Step 3)
pub fn suggestions_routes() -> Router<AppState> {
    use crate::handlers::property_search::get_suggestions_handler;
    Router::new().route("/api/v1/search/suggestions", get(get_suggestions_handler))
}

/// CareCrew Module (Step 4)
pub mod carecrew;
pub use carecrew::{bookings_routes, carecrew_routes};

/// CareCrew Tickets (Support Module)
pub fn carecrew_ticket_routes() -> Router<AppState> {
    use crate::handlers::carecrew_tickets::{
        add_comment_handler, create_ticket_handler, get_ticket_handler, list_tickets_handler,
        update_ticket_handler,
    };
    use axum::routing::patch;
    Router::new()
        .route("/api/v1/carecrew/tickets", post(create_ticket_handler))
        .route("/api/v1/carecrew/tickets", get(list_tickets_handler))
        .route(
            "/api/v1/carecrew/tickets/{ticketId}",
            get(get_ticket_handler),
        )
        .route(
            "/api/v1/carecrew/tickets/{ticketId}",
            patch(update_ticket_handler),
        )
        .route(
            "/api/v1/carecrew/tickets/{ticketId}/comments",
            post(add_comment_handler),
        )
}

/// Recent Chats (JWT-protected)
pub fn recent_chats_routes() -> Router<AppState> {
    use crate::handlers::recent_chats::get_recent_chats;
    Router::new().route("/api/v1/chats/recent", get(get_recent_chats))
}

/// Saved Properties (JWT-protected)
pub fn saved_properties_routes() -> Router<AppState> {
    use crate::handlers::saved_properties::{get_saved_properties, save_property, unsave_property};
    use axum::routing::delete;
    Router::new()
        .route("/api/v1/properties/{id}/save", post(save_property))
        .route("/api/v1/properties/{id}/save", delete(unsave_property))
        .route(
            "/api/v1/users/me/saved-properties",
            get(get_saved_properties),
        )
}

/// Notifications (JWT-protected)
pub fn notifications_routes() -> Router<AppState> {
    use crate::handlers::notifications::{get_notifications, mark_notification_read};
    use axum::routing::patch;
    Router::new()
        .route("/api/v1/notifications", get(get_notifications))
        .route(
            "/api/v1/notifications/{id}/read",
            patch(mark_notification_read),
        )
}

/// Property Filter (JWT-protected)
pub fn property_filter_routes() -> Router<AppState> {
    use crate::handlers::property_filter::filter_properties;
    Router::new().route("/api/v1/properties", get(filter_properties))
}

/// Community APIs (JWT-protected)
pub mod community;
pub use community::community_routes;

/// Moderation APIs (JWT-protected)
pub fn moderation_routes() -> Router<AppState> {
    use crate::handlers::moderation::{
        archive_chat, block_user, report_entity, unarchive_chat, unblock_user,
    };
    Router::new()
        .route(
            "/api/v1/users/{id}/block",
            post(block_user).delete(unblock_user),
        )
        .route("/api/v1/users/{id}/unblock", post(unblock_user))
        .route("/api/v1/report", post(report_entity))
        .route(
            "/api/v1/chats/{id}/archive",
            post(archive_chat).delete(unarchive_chat),
        )
        .route("/api/v1/chats/{id}/unarchive", post(unarchive_chat))
}

/// Vibe APIs (JWT-protected)
pub fn vibes_routes() -> Router<AppState> {
    use crate::handlers::vibes::{accept_vibe, get_matches, reject_vibe, send_vibe};
    Router::new()
        .route("/api/v1/vibes", post(send_vibe))
        .route("/api/v1/vibes/matches", get(get_matches))
        .route("/api/v1/vibes/{id}/accept", post(accept_vibe))
        .route("/api/v1/vibes/{id}/reject", post(reject_vibe))
}

/// Language APIs (JWT-protected)
pub fn language_routes() -> Router<AppState> {
    use crate::handlers::language::{get_languages, set_preferred_language};
    use axum::routing::patch;
    Router::new()
        .route("/api/v1/languages", get(get_languages))
        .route("/api/v1/users/me/language", patch(set_preferred_language))
}

/// Expo Event APIs (JWT-protected)
pub mod expo;
pub use expo::expo_routes;

/// Service Provider Listing APIs
pub mod service_listing;
pub use service_listing::service_listing_routes;

/// CareCrew Review APIs
pub fn carecrew_review_routes() -> Router<AppState> {
    use crate::handlers::carecrew_reviews::{
        create_review, delete_review, edit_review, get_provider_reviews, reply_to_review,
    };
    Router::new()
        .route("/api/reviews/carecrew", post(create_review))
        .route(
            "/api/reviews/carecrew/{id}",
            get(get_provider_reviews)
                .put(edit_review)
                .delete(delete_review),
        )
        .route("/api/reviews/carecrew/{id}/reply", post(reply_to_review))
}

/// Property Review APIs
pub fn property_review_routes() -> Router<AppState> {
    use crate::handlers::property_reviews::{
        create_review, delete_review, edit_review, get_property_reviews, reply_to_review,
    };
    Router::new()
        .route("/api/reviews/property", post(create_review))
        .route(
            "/api/reviews/property/{id}",
            get(get_property_reviews)
                .put(edit_review)
                .delete(delete_review),
        )
        .route("/api/reviews/property/{id}/reply", post(reply_to_review))
}

/// Analytics APIs (public)
pub fn analytics_routes() -> Router<AppState> {
    use crate::handlers::analytics::{get_rent_comparison, get_rent_heatmap, get_rent_trends};
    Router::new()
        .route("/api/v1/analytics/rent-trends", get(get_rent_trends))
        .route("/api/v1/analytics/rent-heatmap", get(get_rent_heatmap))
        .route(
            "/api/v1/analytics/rent-comparison",
            get(get_rent_comparison),
        )
}

pub mod chat_routes;
pub use chat_routes::chat_routes;

pub mod kyc;
pub use kyc::kyc_routes;

pub mod chat;
pub mod visit;

pub mod unified_listing;
pub use unified_listing::unified_listing_routes;

/// Property Share (public, no auth)
pub fn share_routes() -> Router<AppState> {
    Router::new()
        .route("/share/property/{id}", get(crate::handlers::share::share_property))
        .route("/share/news/{id}", get(crate::handlers::share::share_news))
        .route("/share/expo/{id}", get(crate::handlers::share::share_expo))
        .route("/share/carecrew/{id}", get(crate::handlers::share::share_carecrew))
}

pub fn news_routes() -> Router<AppState> {
    use crate::handlers::news::{
        admin_action_news, create_news, get_news, track_news_action, update_news,
        user_create_news, like_news, unlike_news, save_news, unsave_news, report_news, add_comment, get_comments
    };
    use axum::routing::patch;
    Router::new()
        // Public endpoints
        .route("/api/v1/news", get(get_news))
        .route("/api/v1/news", post(user_create_news))
        .route("/api/v1/news/{id}/action", post(track_news_action))
        // Interactions
        .route("/api/v1/news/{id}/like", post(like_news).delete(unlike_news))
        .route("/api/v1/news/{id}/save", post(save_news).delete(unsave_news))
        .route("/api/v1/news/{id}/report", post(report_news))
        .route("/api/v1/news/{id}/comments", post(add_comment).get(get_comments))
        // Image Upload
        .route("/api/v1/news/upload/images", post(crate::handlers::listing_image::upload_listing_images).layer(axum::extract::DefaultBodyLimit::disable()))
        // Admin endpoints (in real app should have admin auth layer)
        .route("/api/v1/admin/news", post(create_news))
        .route("/api/v1/admin/news/{id}", put(update_news))
        .route("/api/v1/admin/news/{id}/action", patch(admin_action_news))
}

pub fn admin_stats_routes() -> Router<AppState> {
    use crate::handlers::admin_stats::{get_stats, get_stats_location, get_stats_trend};
    use axum::routing::get;
    Router::new()
        .route("/api/admin/stats", get(get_stats))
        .route("/api/admin/stats/trend", get(get_stats_trend))
        .route("/api/admin/stats/location", get(get_stats_location))
}

pub fn admin_analytics_routes() -> Router<AppState> {
    use crate::handlers::admin_analytics::{get_engagement, get_kyc_funnel, get_rent_trends};
    use axum::routing::get;
    Router::new()
        .route("/api/admin/analytics/rent-trends", get(get_rent_trends))
        .route("/api/admin/analytics/engagement", get(get_engagement))
        .route("/api/admin/analytics/kyc", get(get_kyc_funnel))
}

pub mod admin_users;
pub use admin_users::admin_users_routes;

pub mod admin_properties;
pub use admin_properties::admin_properties_routes;
