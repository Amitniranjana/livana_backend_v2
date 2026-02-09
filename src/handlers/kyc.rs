use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use axum_extra::extract::Multipart;
use crate::app_state::AppState;
use crate::models::kyc::KycDocType;
use serde_json::json;
use uuid::Uuid;

pub async fn submit_kyc(
    State(state): State<AppState>,
    // Extension(user): Extension<User>, // Assuming authentication middleware provides this
    // For now, let's assume we extract user_id from the multipart for simplicity if auth middleware isn't totally clear,
    // but the spec said "Assume user is already authenticated (user_id is trusted input)".
    // Usually it comes from `Extension<User>` or `Claims`.
    // I'll assume we pass it in multipart for now based on "Fields: user_id: UUID" in spec.
    // SPEC SAYS: "Fields: user_id: UUID" in request. So I read it from multipart.
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut user_id: Option<Uuid> = None;
    let mut email: Option<String> = None;
    let mut name: Option<String> = None;
    let mut doc_type: Option<KycDocType> = None;
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut file_ext: Option<String> = None;

    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name_field = field.name().unwrap_or("").to_string();

        if name_field == "document" {
            let file_name = field.file_name().unwrap_or("doc.pdf").to_string();
             // Simple extraction of extension
            let ext = std::path::Path::new(&file_name)
                .extension()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or("pdf")
                .to_string();
            file_ext = Some(ext);

            match field.bytes().await {
                Ok(bytes) => file_bytes = Some(bytes.to_vec()),
                Err(_) => return (StatusCode::BAD_REQUEST, Json(json!({"error": "Failed to read file"}))).into_response(),
            }
        } else {
            match field.text().await {
                Ok(text) => {
                    match name_field.as_str() {
                        "user_id" => user_id = Uuid::parse_str(&text).ok(),
                        "email" => email = Some(text.to_string()),
                        "name" => name = Some(text.to_string()),
                        "doc_type" => {
                            // Map string to enum
                            doc_type = match text.to_uppercase().as_str() {
                                "AADHAAR" => Some(KycDocType::Aadhaar),
                                "PAN" => Some(KycDocType::Pan),
                                "PASSPORT" => Some(KycDocType::Passport),
                                _ => Some(KycDocType::Other),
                            };
                        }
                        _ => {}
                    }
                },
                Err(_) => {}
            }
        }
    }

    // Validation
    if user_id.is_none() || email.is_none() || name.is_none() || doc_type.is_none() || file_bytes.is_none() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Missing required fields"}))).into_response();
    }

    // File validation (size 10MB)
    let bytes = file_bytes.unwrap();
    if bytes.len() > 10 * 1024 * 1024 {
         return (StatusCode::BAD_REQUEST, Json(json!({"error": "File too large"}))).into_response();
    }

    match state.kyc_service.submit_kyc(
        user_id.unwrap(),
        email.unwrap(),
        name.unwrap(),
        doc_type.unwrap(),
        bytes,
        file_ext.unwrap_or_else(|| "pdf".to_string()),
    ).await {
        Ok(submission) => (StatusCode::CREATED, Json(submission)).into_response(),
        Err(e) => {
            log::error!("KYC Submission error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response()
        }
    }
}
