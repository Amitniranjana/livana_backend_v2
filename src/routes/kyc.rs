use crate::app_state::AppState;
use crate::handlers::kyc::{
    delete_upload, extract_kyc_document, get_kyc, get_kyc_status, submit_kyc, update_kyc,
    upload_document, upload_experience, upload_profile, verify_face_match,
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
        // Advanced KYC (OCR & Face Match)
        .route("/api/v1/kyc/documents/{doc_id}/extract", post(extract_kyc_document))
        .route("/api/v1/kyc/face-match", post(verify_face_match))
}
