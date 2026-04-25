use crate::app_state::AppState;
use crate::handlers::community::{
    create_community, create_community_post, edit_community, edit_community_post,
    get_communities, join_community,
};

use axum::{Router, routing::{post, put}};

pub fn community_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/communities",
            post(create_community).get(get_communities),
        )
        .route("/api/v1/communities/{id}", put(edit_community))
        .route("/api/v1/communities/{id}/join", post(join_community))
        .route(
            "/api/v1/communities/{id}/posts",
            post(create_community_post),
        )
        .route(
            "/api/v1/communities/{community_id}/posts/{post_id}",
            put(edit_community_post),
        )
}
