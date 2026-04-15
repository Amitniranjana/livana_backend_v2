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
    log::info!("=== upload_listing_images called by user: {} ===", auth.user_id);

    let mut files = Vec::new();
    let mut temp_session_id = None;
    let mut listing_type = "property".to_string(); // default fallback
    let mut field_count = 0;

    // Loop through boundary fields
    while let Ok(Some(field)) = multipart.next_field().await {
        field_count += 1;
        let name = field.name().unwrap_or("").to_string();
        let file_name = field.file_name().map(|s| s.to_string());
        let content_type_header = field.content_type().map(|s| s.to_string());

        log::info!(
            "  Field #{}: name={:?}, file_name={:?}, content_type={:?}",
            field_count, name, file_name, content_type_header
        );

        if name == "temp_session_id" {
            if let Ok(text) = field.text().await {
                log::info!("    -> temp_session_id = {}", text);
                temp_session_id = Some(text);
            }
        } else if name == "listing_type" {
            if let Ok(text) = field.text().await {
                log::info!("    -> listing_type = {}", text);
                listing_type = text;
            }
        } else if file_name.is_some() || name == "files" || name == "images" || name.starts_with("file") || name.starts_with("image") {
            // This is a file upload field
            // Max 10 images constraint
            if files.len() >= 10 {
                log::warn!("    -> Rejected: max 10 images already reached");
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "success": false,
                        "message": "Max 10 images allowed"
                    }))
                ).into_response();
            }

            let filename = file_name.unwrap_or_else(|| "upload.jpg".to_string());
            let content_type = content_type_header.unwrap_or_else(|| "image/jpeg".to_string());

            // Validate format
            if !content_type.starts_with("image/jpeg") &&
               !content_type.starts_with("image/png") &&
               !content_type.starts_with("image/webp") {
                log::warn!("    -> Rejected: unsupported content_type={}", content_type);
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "success": false,
                        "message": format!("Only JPEG, PNG, and WEBP formats are supported. Got: {}", content_type)
                    }))
                ).into_response();
            }

            // Extract bytes
            match field.bytes().await {
                Ok(bytes) => {
                    let size = bytes.len();
                    log::info!("    -> Read {} bytes for file: {}", size, filename);
                    // Optional extra safeguard size constraint (5MB limit inside iteration)
                    if size > 5 * 1024 * 1024 {
                        log::warn!("    -> Rejected: file too large ({} bytes)", size);
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({
                                "success": false,
                                "message": format!("File {} exceeds max size of 5MB", filename)
                            }))
                        ).into_response();
                    }
                    if size == 0 {
                        log::warn!("    -> Skipping empty file: {}", filename);
                        continue;
                    }
                    files.push((filename, content_type, bytes, size as i64));
                }
                Err(e) => {
                    log::error!("    -> Error reading file stream: {}", e);
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({
                            "success": false,
                            "message": format!("Error reading file stream: {}", e)
                        }))
                    ).into_response();
                }
            }
        } else {
            log::warn!("    -> Unknown/ignored field: name={:?}", name);
            // Consume the field to avoid blocking the stream
            let _ = field.bytes().await;
        }
    }

    log::info!("Multipart parsing done. Total fields={}, files collected={}", field_count, files.len());

    if files.is_empty() {
        log::warn!("No files found in multipart request (parsed {} fields total)", field_count);
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "message": format!("No files uploaded. Parsed {} fields but none matched a file upload.", field_count)
            }))
        ).into_response();
    }

    let session_id = temp_session_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let user_uuid = Uuid::parse_str(&auth.user_id).unwrap_or_default();
    
    let mut uploaded_images = Vec::new();
    let total_files = files.len() as i32;

    log::info!("Starting S3 uploads: {} files, session={}, type={}", total_files, session_id, listing_type);

    for (i, (filename, content_type, bytes, size)) in files.into_iter().enumerate() {
        let image_id = Uuid::new_v4();
        let key = format!("listings/{}/{}/{}_{}", listing_type, session_id, image_id, filename);

        log::info!("  Uploading [{}/{}] key={} size={}", i + 1, total_files, key, size);

        match app_state.public_storage_service.upload_file(&key, bytes.to_vec(), &content_type).await {
            Ok(()) => {
                log::info!("  S3 upload OK for {}", filename);
            }
            Err(e) => {
                log::error!("  S3 upload FAILED for {}: {}", filename, e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "message": format!("S3 upload failed for {}: {}", filename, e)
                    }))
                ).into_response();
            }
        }

        let bucket_name = std::env::var("PUBLIC_BUCKET_NAME").unwrap_or_else(|_| "livana-public-listings".to_string());
        let aws_region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let url = format!("https://{}.s3.{}.amazonaws.com/{}", bucket_name, aws_region, key);

        let uploaded_at = Utc::now();
        let order_index = (i + 1) as i32;

        match sqlx::query(
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
        .await {
            Ok(_) => log::info!("  DB insert OK for image_id={}", image_id),
            Err(e) => log::error!("  DB insert FAILED for image_id={}: {} (continuing anyway)", image_id, e),
        }

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

    log::info!("Upload complete. {} images uploaded successfully.", uploaded_images.len());

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
