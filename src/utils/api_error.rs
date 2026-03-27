use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
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
            ApiError::Forbidden(msg) => (
                StatusCode::FORBIDDEN,
                msg,
                "FORBIDDEN".to_string(),
                None,
            ),
            ApiError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                msg,
                "NOT_FOUND".to_string(),
                None,
            ),
            ApiError::Conflict(msg) => (
                StatusCode::CONFLICT,
                msg,
                "CONFLICT".to_string(),
                None,
            ),
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
            ApiError::CustomError(status, msg, code) => (
                status,
                msg,
                code,
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

