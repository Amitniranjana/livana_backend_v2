use crate::app_state::AppState;
use crate::handlers::community::{
    create_community, create_community_post, edit_community, edit_community_post, get_communities,
    join_community, delete_community_post, get_community_feed,
};

use axum::{
    Router,
    routing::{get, post, put},
};

pub fn community_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/communities",
            post(create_community).get(get_communities),
        )
        .route("/api/v1/communities/feed", get(get_community_feed))
        .route("/api/v1/communities/{id}", put(edit_community))
        .route("/api/v1/communities/{id}/join", post(join_community))
        .route(
            "/api/v1/communities/{id}/posts",
            post(create_community_post),
        )
        .route(
            "/api/v1/communities/{community_id}/posts/{post_id}",
            put(edit_community_post).delete(delete_community_post),
        )
        .route(
            "/api/v1/communities/upload/images",
            post(crate::handlers::listing_image::upload_listing_images)
                .layer(axum::extract::DefaultBodyLimit::disable()),
        )
}
