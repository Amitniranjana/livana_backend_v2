use axum::{
    http::StatusCode,
    response::Json,
    extract::{State, Json as ExtractJson},
};
use crate::app_state::AppState;
use crate::dtos::request::{ForgotPasswordRequest, ResetPasswordRequest, SigninRequest, SignupRequest};
use serde_json::json;
use crate::utils::util::hash_string;
use crate::utils::auth::create_jwt;
use crate::otp::{generate_otp, send_sms_otp, send_email_otp};

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
pub async fn signup(
    State(app_state): State<AppState>,
    ExtractJson(payload): ExtractJson<SignupRequest>,
) -> impl axum::response::IntoResponse {
    let hashed_password = hash_string(&payload.password);

    // 2. Create user in database
    let user_result = app_state.user_service
        .create_user(
            &payload.first_name,
            &payload.last_name,
            &payload.email,
            &payload.phone_no,
            &hashed_password,
            &payload.gender,
        )
        .await;

    let user = match user_result {
        Ok(user) => user,
        Err(e) => {
            let error_msg = e.to_string();

            // Check if user already exists
            if error_msg.contains("duplicate") || error_msg.contains("already exists") {
                let response = json!({
                    "success": false,
                    "message": "User with this email already exists",
                    "data": null
                });
                return (StatusCode::CONFLICT, Json(response));
            }

            let response = json!({
                "success": false,
                "message": format!("Failed to create user: {}", error_msg),
                "data": null
            });
            return (StatusCode::BAD_REQUEST, Json(response));
        }
    };

    // 3. Generate JWT token
    let token = match create_jwt(&user.id.to_string(), &app_state.jwt_secret, 24) {
        Ok(token) => token,
        Err(e) => {
            let response = json!({
                "success": false,
                "message": format!("Failed to generate token: {}", e),
                "data": null
            });
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
        }
    };

    // 4. Return response
    let response = json!({
        "success": true,
        "message": "User created successfully",
        "data": {
            "token": token,
            "user": {
                "id": user.id,
                "first_name": user.first_name,
                "last_name": user.last_name,
                "email": user.email,
                "phone_no": user.phone_no,
                "user_role": user.user_role,
                "verified": user.verified,
                "status": user.status,
                "created_at": user.created_at
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
    State(app_state): State<AppState>,
    ExtractJson(payload): ExtractJson<SigninRequest>,
) -> impl axum::response::IntoResponse {
    // 1. Find user by email
    let user = match app_state.user_service.find_by_email(&payload.email).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            let response = json!({
                "success": false,
                "message": "Invalid email or password",
                "data": null
            });
            return (StatusCode::UNAUTHORIZED, Json(response));
        }
        Err(e) => {
            let response = json!({
                "success": false,
                "message": format!("Failed to find user: {}", e),
                "data": null
            });
            return (StatusCode::BAD_REQUEST, Json(response));
        }
    };

    // 2. Verify password
    let is_valid = crate::utils::auth::verify_password(&user.password, &payload.password);
    if !is_valid {
        let response = json!({
            "success": false,
            "message": "Invalid email or password",
            "data": null
        });
        return (StatusCode::UNAUTHORIZED, Json(response));
    }

    // Check if user is active
    if user.status != "active" {
        let response = json!({
            "success": false,
            "message": "Account is not active",
            "data": null
        });
        return (StatusCode::UNAUTHORIZED, Json(response));
    }

    // 3. Generate JWT token
    let token = match create_jwt(&user.id.to_string(), &app_state.jwt_secret, 24) {
        Ok(token) => token,
        Err(e) => {
            let response = json!({
                "success": false,
                "message": format!("Failed to generate token: {}", e),
                "data": null
            });
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
        }
    };

    // 4. Return response
    let response = json!({
        "success": true,
        "message": "User signed in successfully",
        "data": {
            "token": token,
            "user": {
                "id": user.id,
                "first_name": user.first_name,
                "last_name": user.last_name,
                "email": user.email,
                "phone_no": user.phone_no,
                "user_role": user.user_role,
                "verified": user.verified,
                "status": user.status,
                "created_at": user.created_at
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
    // 1. Add token to blacklist (if using blacklist)
    // Extract token from request headers if needed
    // app_state.token_blacklist.add_token(token).await;

    // 2. Return success (stateless JWT approach)
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
    State(app_state): State<AppState>,
    ExtractJson(payload): ExtractJson<ForgotPasswordRequest>,
) -> impl axum::response::IntoResponse {
    // 1. Find user by email
    let user = match app_state.user_service.find_by_email(&payload.email).await {

        Ok(Some(user)) => user,
        Ok(None) => {
            let response = json!({
                "success": false,
                "message": "User not found with this email",
                "data": null
            });
            return (StatusCode::NOT_FOUND, Json(response));
        }
        Err(e) => {
            let response = json!({
                "success": false,
                "message": format!("Failed to find user: {}", e),
                "data": null
            });
            return (StatusCode::BAD_REQUEST, Json(response));
        }
    };
    // 2. Generate reset code (simple random uuid for now)
    let reset_code = uuid::Uuid::new_v4().to_string();

    // 3. Store reset code in memory with expiration (if needed)
    if let Err(e) = app_state.user_service
        .store_reset_code(&user.id.to_string(), &reset_code)
        .await
    {
        let response = json!({
            "success": false,
            "message": format!("Failed to store reset code: {}", e),
            "data": null
        });
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
    }
    // Try sending reset code via email (best effort)
    if let Err(_) = send_email_otp(&user.email, &reset_code).await {
        // If email fails, we still return error because user expects a reset delivery
        let response = json!({
            "success": false,
            "message": "Failed to send reset code via email",
            "data": null
        });
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
    }
    // 5. Return response
    let response = json!({
        "success": true,
        "message": "Reset link generated and stored",
        "data": {
            "email": user.email,
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
    State(app_state): State<AppState>,
    ExtractJson(payload): ExtractJson<ResetPasswordRequest>,
) -> impl axum::response::IntoResponse {
    // 1. Verify reset code
    let user_id = match app_state.user_service
        .verify_reset_code(&payload.code)
        .await
    {
        Ok(Some(user_id)) => user_id,
        Ok(None) => {
            let response = json!({
                "success": false,
                "message": "Invalid or expired reset code",
                "data": null
            });
            return (StatusCode::BAD_REQUEST, Json(response));
        }
        Err(e) => {
            let response = json!({
                "success": false,
                "message": format!("Failed to verify reset code: {}", e),
                "data": null
            });
            return (StatusCode::BAD_REQUEST, Json(response));
        }
    };

    // 2. Hash new password
    let hashed_password = hash_string(&payload.new_password);

    // 3. Update user password
    if let Err(e) = app_state.user_service
        .update_password(&user_id, &hashed_password)
        .await
    {
        let response = json!({
            "success": false,
            "message": format!("Failed to update password: {}", e),
            "data": null
        });
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
    }

    // 4. Delete used reset code
    let _ = app_state.user_service.delete_reset_code(&payload.code).await;

    // 5. Return response
    let response = json!({
        "success": true,
        "message": "Password reset successfully",
        "data": {
            "password_updated": true,
            "user_id": user_id
        }
    });

    (StatusCode::OK, Json(response))
}