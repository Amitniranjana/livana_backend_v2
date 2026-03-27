use crate::app_state::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};

use crate::dtos::request::UpdateProfileRequest;
use crate::dtos::response::ApiResponse;
use crate::utils::auth_extractor::AuthenticationUser;
use axum_extra::extract::Multipart;
use serde_json::json;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

/// Get user profile
#[utoipa::path(
    get,
    path = "/api/user/profile",
    responses(
        (status = 200, description = "User profile retrieved successfully", body = ApiResponse<UserResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "User not found")
    ),
    tag = "User Profile"
)]
pub async fn get_profile(
    State(app_state): State<AppState>,
    auth_user: AuthenticationUser,
) -> impl axum::response::IntoResponse {
    let result = app_state
        .user_service
        .get_user_profile(&auth_user.user_id)
        .await;

    match result {
        Ok(user_response) => {
            let response = ApiResponse {
                success: true,
                message: "User profile retrieved successfully".to_string(),
                data: user_response,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response = json!({
                "success": false,
                "message": e,
                "data": null
            });
            (StatusCode::NOT_FOUND, Json(response)).into_response()
        }
    }
}

/// Update user profile
#[utoipa::path(
    put,
    path = "/api/user/profile",
    request_body = UpdateProfileRequest,
    responses(
        (status = 200, description = "Profile updated successfully", body = ApiResponse<UserResponse>),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "User not found")
    ),
    tag = "User Profile"
)]
pub async fn update_profile(
    State(app_state): State<AppState>,
    auth_user: AuthenticationUser,
    Json(payload): Json<UpdateProfileRequest>,
) -> impl axum::response::IntoResponse {
    let result = app_state
        .user_service
        .update_user_profile(&auth_user.user_id, payload)
        .await;

    match result {
        Ok(user_response) => {
            let response = ApiResponse {
                success: true,
                message: "Profile updated successfully".to_string(),
                data: user_response,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response = json!({
                "success": false,
                "message": format!("Failed to update profile: {}", e),
                "data": null
            });
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
        }
    }
}

/// Upload profile image
#[utoipa::path(
    post,
    path = "/api/user/profile/upload-image",
    responses(
        (status = 200, description = "Profile image uploaded successfully", body = ApiResponse<serde_json::Value>),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 413, description = "File too large")
    ),
    tag = "User Profile"
)]
pub async fn upload_profile_image(
    State(app_state): State<AppState>,
    auth_user: AuthenticationUser,
    mut multipart: Multipart,
) -> impl axum::response::IntoResponse {
    let mut image_url: Option<String> = None;

    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or("").to_string();

        if name == "file" || name == "image" {
            let file_name = field.file_name().unwrap_or("image.jpg").to_string();
            let content_type = field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_string();

            // Basic validation
            if !content_type.starts_with("image/") {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"success": false, "message": "Invalid file type"})),
                )
                    .into_response();
            }

            let data = field.bytes().await.unwrap_or_default();
            if data.len() > 5 * 1024 * 1024 {
                // 5MB limit
                return (
                    StatusCode::PAYLOAD_TOO_LARGE,
                    Json(json!({"success": false, "message": "File too large (max 5MB)"})),
                )
                    .into_response();
            }

            // Ensure uploads directory exists
            if let Err(e) = tokio::fs::create_dir_all("uploads").await {
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": format!("Failed to create uploads dir: {}", e)}))).into_response();
            }

            // Generate unique filename
            let ext = Path::new(&file_name)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("jpg");
            let new_filename = format!("{}_{}.{}", auth_user.user_id, uuid::Uuid::new_v4(), ext);
            let filepath = format!("uploads/{}", new_filename);

            // Save file
            let mut file = match File::create(&filepath).await {
                Ok(f) => f,
                Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": format!("Failed to create file: {}", e)}))).into_response(),
            };

            if let Err(e) = file.write_all(&data).await {
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": format!("Failed to write file: {}", e)}))).into_response();
            }

            // Construct URL (Assuming served at /uploads/)
            // Ideally should use a configured base URL
            image_url = Some(format!("/uploads/{}", new_filename));
        }
    }

    if let Some(url) = image_url {
        let result = app_state
            .user_service
            .update_profile_image(&auth_user.user_id, &url)
            .await;
        match result {
            Ok(_) => {
                 let response = json!({
                    "success": true,
                    "message": "Profile image uploaded successfully",
                    "data": {
                        "image_url": url
                    }
                });
                (StatusCode::OK, Json(response)).into_response()
            }
            Err(e) => {
                 (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": format!("Failed to update profile: {}", e)}))).into_response()
            }
        }
    } else {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "No file uploaded"})),
        )
            .into_response()
    }
}
