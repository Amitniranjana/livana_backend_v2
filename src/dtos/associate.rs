use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct RegisterAssociateDto {
    pub name: String,
    pub email: String,
    pub phone: String,
    pub password: String,
    pub associate_type: Option<String>,
    pub gender: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct AssociateRegistrationResponse {
    pub associate_id: Uuid,
    pub status: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct KycUploadDto {
    pub aadhaar_url: String,
    pub pan_url: String,
    pub business_license_url: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct AssociateTypeDto {
    pub id: Uuid,
    pub name: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct AssociateProfileDto {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub kbc: String,
    pub associate_type: Option<String>,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
