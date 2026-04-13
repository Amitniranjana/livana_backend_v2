use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct UploadedImage {
    pub image_id: String,
    pub url: String,
    pub filename: String,
    pub size: i64,
    pub mime_type: String,
    pub order: i32,
    pub uploaded_at: String,
}

#[derive(Debug, Serialize)]
pub struct UploadedImagesData {
    pub uploaded_images: Vec<UploadedImage>,
    pub total_uploaded: i32,
    pub temp_session_id: String,
}

#[derive(Debug, Serialize)]
pub struct ListingImageResponse {
    pub success: bool,
    pub message: String,
    pub data: UploadedImagesData,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub message: String,
}
