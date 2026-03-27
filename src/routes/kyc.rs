use crate::app_state::AppState;
use crate::handlers::kyc::{
    delete_upload, get_kyc, get_kyc_status, submit_kyc, update_kyc, upload_document,
    upload_experience, upload_profile,
};
use axum::{
    Router,
    routing::{delete, get, post},
};

pub fn kyc_routes() -> Router<AppState> {
    Router::new()
        // File uploads (multipart)
        .route("/api/kyc/upload/profile", post(upload_profile))
        .route("/api/kyc/upload/document", post(upload_document))
        .route("/api/kyc/upload/experience", post(upload_experience))
        .route("/api/kyc/upload/{file_id}", delete(delete_upload))
        // KYC data — status must come BEFORE {id} to avoid shadowing
        .route("/api/kyc/submit", post(submit_kyc))
        .route("/api/kyc/status/{user_id}", get(get_kyc_status))
        // GET + PUT share the same path pattern — differentiated by HTTP method
        .route("/api/kyc/{id}", get(get_kyc).put(update_kyc))
}
