use axum::{
    routing::post,
    Router,
};
use crate::app_state::AppState;
use crate::handlers::kyc::submit_kyc;

pub fn kyc_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/kyc/submissions", post(submit_kyc))
}
