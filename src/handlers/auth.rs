use axum::{
    http::StatusCode,
    response::Json,
    extract::State,
};
use crate::app_state::AppState;
use crate::dtos::request::{ForgotPasswordRequest, ResetPasswordRequest, SigninRequest, SignupRequest};
use serde_json::json;
use crate::utils::util::hash_string;
// Auth handlers
/// User registration
#[utoipa::path(
    post,
    path = "/api/auth/signup",
    request_body = SignupRequest,
    responses(
        (status = 201, description = "User created successfully", body = ApiResponse<AuthResponse>),
        (status = 400, description = "Bad request"),
        (status = 409, description = "User already exists")
    ),
    tag = "Authentication"
)]

#[allow(unused_variables)]
pub async fn signup(
    State(app_state): State<AppState>,
    Json(payload): Json<SignupRequest>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement user creation logic
    // 1. Hash password
    let hashed_password=hash_string(payload.password);
    // 2. Create user in database
    //app_state.user_service
    // 3. Generate JWT token
    // 4. Return response

    let response = json!({
        "success": true,
        "message": "User created successfully",
        "data": {
            "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
            "user": {
                "id": "123e4567-e89b-12d3-a456-426614174000",
                "first_name": "John",
                "last_name": "Doe",
                "email": "john.doe@example.com",
                "phone_no": "1234567890",
                "gender": "male",
                "user_role": "user",
                "verified": false,
                "profile_image_url": null,
                "bio": null,
                "business_name": null,
                "license_number": null,
                "experience_years": null,
                "commission_rate": null,
                "broker_rating": null,
                "total_reviews": null,
                "is_verified_broker": false,
                "status": "active",
                "created_at": "2024-01-01T00:00:00Z"
            }
        }
    });

    (StatusCode::CREATED, Json(response))
}

/// User login
#[utoipa::path(
    post,
    path = "/api/auth/signin",
    request_body = SigninRequest,
    responses(
        (status = 200, description = "User signed in successfully", body = ApiResponse<AuthResponse>),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Invalid credentials")
    ),
    tag = "Authentication"
)]
pub async fn signin(
    State(_app_state): State<AppState>,
    Json(_payload): Json<SigninRequest>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement user authentication logic
    // 1. Find user by email
    // 2. Verify password
    // 3. Generate JWT token
    // 4. Return response

    let response = json!({
        "success": true,
        "message": "User signed in successfully",
        "data": {
            "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
            "user": {
                "id": "123e4567-e89b-12d3-a456-426614174000",
                "first_name": "John",
                "last_name": "Doe",
                "email": "john.doe@example.com",
                "phone_no": "1234567890",
                "gender": "male",
                "user_role": "user",
                "verified": true,
                "profile_image_url": "https://example.com/profile.jpg",
                "bio": "Software developer with 5 years of experience",
                "business_name": null,
                "license_number": null,
                "experience_years": null,
                "commission_rate": null,
                "broker_rating": null,
                "total_reviews": null,
                "is_verified_broker": false,
                "status": "active",
                "created_at": "2024-01-01T00:00:00Z"
            }
        }
    });

    (StatusCode::OK, Json(response))
}

/// User logout
#[utoipa::path(
    post,
    path = "/api/auth/signout",
    responses(
        (status = 200, description = "User signed out successfully", body = ApiResponse<()>),
        (status = 401, description = "Unauthorized")
    ),
    tag = "Authentication"
)]
pub async fn signout(
    State(_app_state): State<AppState>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement logout logic
    // 1. Add token to blacklist (if using blacklist)
    // 2. Or just return success (stateless JWT)

    let response = json!({
        "success": true,
        "message": "User signed out successfully",
        "data": null
    });

    (StatusCode::OK, Json(response))
}

/// Send forgot password link
#[utoipa::path(
    post,
    path = "/api/auth/send-forgot-password-link",
    request_body = ForgotPasswordRequest,
    responses(
        (status = 200, description = "Reset link sent successfully", body = ApiResponse<serde_json::Value>),
        (status = 400, description = "Bad request"),
        (status = 404, description = "User not found")
    ),
    tag = "Authentication"
)]
pub async fn send_forgot_password_link(
    State(_app_state): State<AppState>,
    Json(_payload): Json<ForgotPasswordRequest>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement forgot password logic
    // 1. Find user by email
    // 2. Generate reset code
    // 3. Send email with reset link
    // 4. Return response

    let response = json!({
        "success": true,
        "message": "Reset link sent successfully to your email",
        "data": {
            "email": "john.doe@example.com",
            "reset_code_sent": true
        }
    });

    (StatusCode::OK, Json(response))
}

/// Reset password
#[utoipa::path(
    post,
    path = "/api/auth/reset-password",
    request_body = ResetPasswordRequest,
    responses(
        (status = 200, description = "Password reset successfully", body = ApiResponse<serde_json::Value>),
        (status = 400, description = "Bad request"),
        (status = 404, description = "User not found"),
        (status = 400, description = "Invalid reset code")
    ),
    tag = "Authentication"
)]
pub async fn reset_password(
    State(_app_state): State<AppState>,
    Json(_payload): Json<ResetPasswordRequest>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement password reset logic
    // 1. Verify reset code
    // 2. Hash new password
    // 3. Update user password
    // 4. Return response

    let response = json!({
        "success": true,
        "message": "Password reset successfully",
        "data": {
            "password_updated": true,
            "user_id": "123e4567-e89b-12d3-a456-426614174000"
        }
    });

    (StatusCode::OK, Json(response))
}