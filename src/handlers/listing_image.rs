use axum::{
    extract::State,
    http::StatusCode,
    Json,
    response::IntoResponse,
};
use serde_json::json;
use uuid::Uuid;
use chrono::Utc;

// Use the multipart from axum_extra as consistent with the rest of the app
use axum_extra::extract::Multipart; 

use crate::{
    app_state::AppState,
    dtos::listing_image::{UploadedImage, UploadedImagesData, ListingImageResponse},
    utils::auth_extractor::AuthenticationUser,
    services::storage::StorageService,
};

pub async fn upload_listing_images(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut files = Vec::new();
    let mut temp_session_id = None;
    let mut listing_type = "property".to_string(); // default fallback

    // Loop through boundary fields
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();

        if name == "temp_session_id" {
            if let Ok(text) = field.text().await {
                temp_session_id = Some(text);
            }
        } else if name == "listing_type" {
            if let Ok(text) = field.text().await {
                listing_type = text;
            }
        } else if name == "files" {
            // Max 10 images constraint
            if files.len() >= 10 {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "success": false,
                        "message": "Max 10 images allowed"
                    }))
                ).into_response();
            }

            let filename = field.file_name().unwrap_or("upload.jpg").to_string();
            let content_type = field.content_type().unwrap_or("image/jpeg").to_string();

            // Validate format
            if !content_type.starts_with("image/jpeg") &&
               !content_type.starts_with("image/png") &&
               !content_type.starts_with("image/webp") {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "success": false,
                        "message": "Only JPEG, PNG, and WEBP formats are supported."
                    }))
                ).into_response();
            }

            // Extract bytes
            match field.bytes().await {
                Ok(bytes) => {
                    let size = bytes.len();
                    // Optional extra safeguard size constraint (5MB limit inside iteration)
                    if size > 5 * 1024 * 1024 {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({
                                "success": false,
                                "message": format!("File {} exceeds max size of 5MB", filename)
                            }))
                        ).into_response();
                    }
                    files.push((filename, content_type, bytes, size as i64));
                }
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({
                            "success": false,
                            "message": format!("Error reading file stream: {}", e)
                        }))
                    ).into_response();
                }
            }
        }
    }

    if files.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "message": "No files uploaded."
            }))
        ).into_response();
    }

    let session_id = temp_session_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let user_uuid = Uuid::parse_str(&auth.user_id).unwrap_or_default();
    
    let mut uploaded_images = Vec::new();
    let total_files = files.len() as i32;

    for (i, (filename, content_type, bytes, size)) in files.into_iter().enumerate() {
        let image_id = Uuid::new_v4();
        let key = format!("listings/{}/{}/{}_{}", listing_type, session_id, image_id, filename);

        if let Err(e) = app_state.storage_service.upload_file(&key, bytes.to_vec(), &content_type).await {
            log::error!("Failed to upload image {}: {}", filename, e);
            // Even if one fails, we can proceed with others to support resilience in batch upload
            continue; 
        }

        let bucket_name = std::env::var("KYC_BUCKET_NAME").unwrap_or_else(|_| "livana-kyc-documents".to_string());
        let aws_region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let url = format!("https://{}.s3.{}.amazonaws.com/{}", bucket_name, aws_region, key);

        let uploaded_at = Utc::now();
        let order_index = (i + 1) as i32;

        let _ = sqlx::query(
            r#"
            INSERT INTO listing_images
            (id, user_id, url, filename, size, mime_type, order_index, temp_session_id, listing_type, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#
        )
        .bind(image_id)
        .bind(user_uuid)
        .bind(&url)
        .bind(&filename)
        .bind(size)
        .bind(&content_type)
        .bind(order_index)
        .bind(&session_id)
        .bind(&listing_type)
        .bind(uploaded_at)
        .execute(&app_state.db)
        .await;

        uploaded_images.push(UploadedImage {
            image_id: image_id.to_string(),
            url,
            filename,
            size,
            mime_type: content_type,
            order: order_index,
            uploaded_at: uploaded_at.to_rfc3339(),
        });
    }

    if uploaded_images.is_empty() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "message": "Failed to upload any images."
            }))
        ).into_response();
    }

    (
        StatusCode::OK,
        Json(ListingImageResponse {
            success: true,
            message: "Listing images uploaded successfully".to_string(),
            data: UploadedImagesData {
                uploaded_images,
                total_uploaded: total_files,
                temp_session_id: session_id,
            }
        })
    ).into_response()
}
