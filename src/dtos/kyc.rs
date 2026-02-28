use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use chrono::{DateTime, Utc};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Pending,
    Verified,
    Rejected,
    PendingReview,
}

impl std::fmt::Display for VerificationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending       => write!(f, "pending"),
            Self::Verified      => write!(f, "verified"),
            Self::Rejected      => write!(f, "rejected"),
            Self::PendingReview => write!(f, "pending_review"),
        }
    }
}

// ---------------------------------------------------------------------------
// Submit KYC Request  (POST /api/kyc/submit)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, ToSchema)]
pub struct KycSubmitRequest {
    // Personal info
    pub full_name: String,
    pub mobile_number: String,
    pub email_id: String,
    pub gender: Option<String>,
    pub date_of_birth: Option<String>,
    pub profile_picture_url: Option<String>,

    // Present address
    pub apartment_name: Option<String>,
    pub street_address: Option<String>,
    pub landmark: Option<String>,
    pub city: Option<String>,
    pub zip_code: Option<String>,
    pub state: Option<String>,
    pub country: Option<String>,

    // Permanent address
    pub permanent_address: Option<String>,
    pub is_permanent_same_as_present: Option<bool>,

    // Geo
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,

    // Government ID
    pub govt_id_type: String,        // "aadhaar" | "pan" | "passport" | "driving_license"
    pub govt_id_number: String,
    pub govt_id_document_url: String, // S3 URL — OCR runs on this

    // Professional
    pub company_name: Option<String>,
    pub services: Option<Vec<String>>,
    pub experience_document_url: Option<String>,
}

// ---------------------------------------------------------------------------
// Update KYC Request  (PUT /api/kyc/{kyc_id})
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, ToSchema)]
pub struct KycUpdateRequest {
    pub full_name: Option<String>,
    pub mobile_number: Option<String>,
    pub email_id: Option<String>,
    pub gender: Option<String>,
    pub date_of_birth: Option<String>,
    pub profile_picture_url: Option<String>,
    pub apartment_name: Option<String>,
    pub street_address: Option<String>,
    pub landmark: Option<String>,
    pub city: Option<String>,
    pub zip_code: Option<String>,
    pub state: Option<String>,
    pub country: Option<String>,
    pub permanent_address: Option<String>,
    pub is_permanent_same_as_present: Option<bool>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub govt_id_type: Option<String>,
    pub govt_id_number: Option<String>,
    pub govt_id_document_url: Option<String>,
    pub company_name: Option<String>,
    pub services: Option<Vec<String>>,
    pub experience_document_url: Option<String>,
}

// ---------------------------------------------------------------------------
// KYC Response
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, ToSchema)]
pub struct KycResponse {
    pub id: String,
    pub user_id: String,
    // Personal
    pub full_name: String,
    pub mobile_number: String,
    pub email_id: String,
    pub gender: Option<String>,
    pub date_of_birth: Option<String>,
    pub profile_picture_url: Option<String>,
    // Present address
    pub apartment_name: Option<String>,
    pub street_address: Option<String>,
    pub landmark: Option<String>,
    pub city: Option<String>,
    pub zip_code: Option<String>,
    pub state: Option<String>,
    pub country: Option<String>,
    // Permanent address
    pub permanent_address: Option<String>,
    pub is_permanent_same_as_present: Option<bool>,
    // Geo
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    // Government ID
    pub govt_id_type: String,
    pub govt_id_number: String,
    pub govt_id_document_url: String,
    // Professional
    pub company_name: Option<String>,
    pub services: Option<Vec<String>>,
    pub experience_document_url: Option<String>,
    // Status
    pub verification_status: String,
    pub submitted_at: Option<String>,
    pub verified_at: Option<String>,
    pub rejection_reason: Option<String>,
}

// ---------------------------------------------------------------------------
// KYC Status Response  (GET /api/kyc/status/{user_id})
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, ToSchema)]
pub struct KycStatusResponse {
    pub user_id: String,
    pub is_verified: bool,
    pub verification_status: String,
    pub submitted_at: Option<String>,
    pub verified_at: Option<String>,
    pub rejection_reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Upload Response  (POST /api/kyc/upload/*)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, ToSchema)]
pub struct UploadResponse {
    pub url: String,
    pub filename: String,
    pub size: u64,
    pub mime_type: String,
    pub uploaded_at: String,
}

// ---------------------------------------------------------------------------
// Internal: raw DB row helper
// ---------------------------------------------------------------------------

pub struct KycRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub full_name: String,
    pub mobile_number: String,
    pub email_id: String,
    pub gender: Option<String>,
    pub date_of_birth: Option<String>,
    pub profile_picture_url: Option<String>,
    pub apartment_name: Option<String>,
    pub street_address: Option<String>,
    pub landmark: Option<String>,
    pub city: Option<String>,
    pub zip_code: Option<String>,
    pub state: Option<String>,
    pub country: Option<String>,
    pub permanent_address: Option<String>,
    pub is_permanent_same_as_present: Option<bool>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub govt_id_type: String,
    pub govt_id_number: String,
    pub govt_id_document_url: String,
    pub company_name: Option<String>,
    pub services_json: Option<serde_json::Value>,
    pub experience_document_url: Option<String>,
    pub verification_status: String,
    pub submitted_at: Option<DateTime<Utc>>,
    pub verified_at: Option<DateTime<Utc>>,
    pub rejection_reason: Option<String>,
}

impl From<KycRow> for KycResponse {
    fn from(r: KycRow) -> Self {
        let services: Option<Vec<String>> = r
            .services_json
            .and_then(|v| serde_json::from_value(v).ok());
        KycResponse {
            id:                          r.id.to_string(),
            user_id:                     r.user_id.to_string(),
            full_name:                   r.full_name,
            mobile_number:               r.mobile_number,
            email_id:                    r.email_id,
            gender:                      r.gender,
            date_of_birth:               r.date_of_birth,
            profile_picture_url:         r.profile_picture_url,
            apartment_name:              r.apartment_name,
            street_address:              r.street_address,
            landmark:                    r.landmark,
            city:                        r.city,
            zip_code:                    r.zip_code,
            state:                       r.state,
            country:                     r.country,
            permanent_address:           r.permanent_address,
            is_permanent_same_as_present: r.is_permanent_same_as_present,
            latitude:                    r.latitude,
            longitude:                   r.longitude,
            govt_id_type:                r.govt_id_type,
            govt_id_number:              r.govt_id_number,
            govt_id_document_url:        r.govt_id_document_url,
            company_name:                r.company_name,
            services,
            experience_document_url:     r.experience_document_url,
            verification_status:         r.verification_status,
            submitted_at:                r.submitted_at.map(|d| d.to_rfc3339()),
            verified_at:                 r.verified_at.map(|d| d.to_rfc3339()),
            rejection_reason:            r.rejection_reason,
        }
    }
}
