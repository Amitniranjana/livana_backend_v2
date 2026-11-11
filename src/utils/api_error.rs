use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use std::collections::HashMap;

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    pub success: bool,
    pub message: String,
    pub error_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<HashMap<String, Vec<String>>>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    ValidationError(HashMap<String, Vec<String>>),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    InternalServerError(String),
    Conflict(String),
    UnprocessableEntity(String),
    /// Custom error: (StatusCode, message, error_code)
    CustomError(StatusCode, String, String),

    // Zero Deposit V2 Shared Error Codes
    NotOwner,
    MakerChecker,
    InvalidState(String),
    InsufficientTokens { requested: f64, available: f64 },
    RedemptionAlreadyOpen,
    DuplicateRequest,
    TokensExpired,
    FileTooLarge,
    UnsupportedFileType,
    DocumentsMissing(Vec<String>),
    TenancyTooShort,
    PersonalEmailNotAllowed,
    PreconditionFailed(Vec<String>),
    BadHmac,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message, error_code, errors) = match self {
            ApiError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                msg,
                "BAD_REQUEST".to_string(),
                None,
            ),
            ApiError::ValidationError(errs) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "Validation failed".to_string(),
                "VALIDATION_ERROR".to_string(),
                Some(errs),
            ),
            ApiError::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                msg,
                "UNAUTHORIZED".to_string(),
                None,
            ),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg, "FORBIDDEN".to_string(), None),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg, "NOT_FOUND".to_string(), None),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, msg, "CONFLICT".to_string(), None),
            ApiError::UnprocessableEntity(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                msg,
                "UNPROCESSABLE_ENTITY".to_string(),
                None,
            ),
            ApiError::InternalServerError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                msg,
                "INTERNAL_SERVER_ERROR".to_string(),
                None,
            ),
            ApiError::CustomError(status, msg, code) => (status, msg, code, None),

            // ZD v2 Shared Errors (Issue 108 / ZD Spec)
            ApiError::NotOwner => (
                StatusCode::FORBIDDEN,
                "Application, redemption or chat belongs to another user".to_string(),
                "NOT_OWNER".to_string(),
                None,
            ),
            ApiError::MakerChecker => (
                StatusCode::FORBIDDEN,
                "Maker-checker rule violation: approver and payer must be different admins".to_string(),
                "MAKER_CHECKER".to_string(),
                None,
            ),
            ApiError::InvalidState(msg) => (
                StatusCode::CONFLICT,
                msg,
                "INVALID_STATE".to_string(),
                None,
            ),
            ApiError::InsufficientTokens { requested, available } => (
                StatusCode::CONFLICT,
                format!("Insufficient available tokens: requested {}, available {}", requested, available),
                "INSUFFICIENT_TOKENS".to_string(),
                None,
            ),
            ApiError::RedemptionAlreadyOpen => (
                StatusCode::CONFLICT,
                "An open redemption already exists for this application".to_string(),
                "REDEMPTION_ALREADY_OPEN".to_string(),
                None,
            ),
            ApiError::DuplicateRequest => (
                StatusCode::CONFLICT,
                "Duplicate request with matching idempotency key and conflicting payload".to_string(),
                "DUPLICATE_REQUEST".to_string(),
                None,
            ),
            ApiError::TokensExpired => (
                StatusCode::GONE,
                "Credit tokens have expired (30-day window passed) or allocation revoked".to_string(),
                "TOKENS_EXPIRED".to_string(),
                None,
            ),
            ApiError::FileTooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "File exceeds maximum allowed size of 10 MB".to_string(),
                "FILE_TOO_LARGE".to_string(),
                None,
            ),
            ApiError::UnsupportedFileType => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "Unsupported file type. Only PDF, JPG, and PNG are allowed".to_string(),
                "UNSUPPORTED_FILE_TYPE".to_string(),
                None,
            ),
            ApiError::DocumentsMissing(docs) => {
                let mut map = HashMap::new();
                map.insert("missing_documents".to_string(), docs);
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "Required documents are missing from checklist".to_string(),
                    "DOCUMENTS_MISSING".to_string(),
                    Some(map),
                )
            }
            ApiError::TenancyTooShort => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "Tenancy agreement term must be at least 11 months".to_string(),
                "TENANCY_TOO_SHORT".to_string(),
                None,
            ),
            ApiError::PersonalEmailNotAllowed => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "Personal email domains are not allowed for employer verification".to_string(),
                "PERSONAL_EMAIL_NOT_ALLOWED".to_string(),
                None,
            ),
            ApiError::PreconditionFailed(keys) => {
                let mut map = HashMap::new();
                map.insert("failing_preconditions".to_string(), keys);
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "Precondition check failed".to_string(),
                    "PRECONDITION_FAILED".to_string(),
                    Some(map),
                )
            }
            ApiError::BadHmac => (
                StatusCode::UNAUTHORIZED,
                "Invalid or expired HMAC signature".to_string(),
                "BAD_HMAC".to_string(),
                None,
            ),
        };

        let body = ApiErrorResponse {
            success: false,
            message,
            error_code,
            errors,
        };

        (status, Json(body)).into_response()
    }
}

#[allow(dead_code)]
impl ApiError {
    pub fn booking_not_completed() -> Self {
        ApiError::CustomError(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Booking is not completed yet".to_string(),
            "BOOKING_NOT_COMPLETED".to_string(),
        )
    }
    pub fn visit_not_completed() -> Self {
        ApiError::CustomError(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Visit is not completed yet".to_string(),
            "VISIT_NOT_COMPLETED".to_string(),
        )
    }
    pub fn review_already_exists() -> Self {
        ApiError::CustomError(
            StatusCode::CONFLICT,
            "A review already exists for this booking/visit".to_string(),
            "REVIEW_ALREADY_EXISTS".to_string(),
        )
    }
    pub fn review_not_found() -> Self {
        ApiError::CustomError(
            StatusCode::NOT_FOUND,
            "Review not found".to_string(),
            "REVIEW_NOT_FOUND".to_string(),
        )
    }
    pub fn edit_period_expired() -> Self {
        ApiError::CustomError(
            StatusCode::FORBIDDEN,
            "Edit/delete period of 30 days has expired".to_string(),
            "EDIT_PERIOD_EXPIRED".to_string(),
        )
    }
    pub fn reply_already_exists() -> Self {
        ApiError::CustomError(
            StatusCode::CONFLICT,
            "A reply already exists for this review".to_string(),
            "REPLY_ALREADY_EXISTS".to_string(),
        )
    }
    pub fn invalid_rating() -> Self {
        ApiError::CustomError(
            StatusCode::BAD_REQUEST,
            "Rating must be between 1.0 and 5.0".to_string(),
            "INVALID_RATING".to_string(),
        )
    }
    pub fn access_denied() -> Self {
        ApiError::CustomError(
            StatusCode::FORBIDDEN,
            "You do not have permission to perform this action".to_string(),
            "ACCESS_DENIED".to_string(),
        )
    }
}
