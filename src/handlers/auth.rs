// src/handlers/auth.rs

use crate::app_state::AppState;
use crate::dtos::request::{
    ChangePasswordRequest, ForgotPasswordRequest, ResetPasswordRequest, SendOtpRequest,
    SigninRequest, SignupRequest, UpdateAssociateTypeRequest, VerifyOtpRequest,
};
use crate::dtos::response::{SignupResponseData, SignupUserData};
use crate::utils::auth::{create_jwt, verify_password};
use crate::utils::auth_extractor::AuthenticationUser;
use crate::utils::util::hash_string;
use axum::{
    extract::{Json as ExtractJson, State},
    http::StatusCode,
    response::IntoResponse,
    response::Json,
};
use serde::Deserialize;
use serde_json::json;

// Import OTP functions
use crate::otp::{generate_otp, send_email_otp, send_sms_otp};

// --- DTO for Testing OTP (Internal use) ---
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct TestOtpRequest {
    pub channel: String, // "sms" or "email"
    pub destination: String,
}

// --- Handlers ---

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
) -> impl IntoResponse {
    let hashed_password = hash_string(&payload.password);

    // 1. Create user in database
    let user_result = app_state
        .user_service
        .create_user(
            &payload.first_name,
            &payload.last_name,
            payload.email.as_deref().unwrap_or(""),
            &payload.phone_no,
            &hashed_password,
            &payload.gender,
            &payload.user_role,
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
                    "message": "User with this email or phone number already exists",
                    "data": serde_json::Value::Null
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

    // 2. Generate JWT token
    let token = match create_jwt(&user.id.to_string(), &app_state.jwt_secret, 360) {
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
    let signup_response = SignupResponseData {
        token,
        user: SignupUserData {
            id: user.id,
            first_name: user.first_name,
            last_name: user.last_name,
            email: user.email,
            phone_no: user.phone_no,
            user_role: user.user_role,
            verified: user.verified,
            is_phone_verified: user.is_phone_verified,
            status: user.status,
            associate_type: user.associate_type,
            created_at: user.created_at,
        },
    };

    let response = json!({
        "success": true,
        "message": "User created successfully.",
        "data": signup_response
    });

    (StatusCode::CREATED, Json(response))
}

/// User login — supports email OR phone-based login
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
) -> impl IntoResponse {
    // 1. Find user by email OR phone
    let user = if let Some(ref email) = payload.email {
        if !email.is_empty() {
            match app_state.user_service.find_by_email(email).await {
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
            }
        } else if let Some(ref phone) = payload.phone_no {
            match app_state.user_service.find_by_phone(phone).await {
                Ok(Some(user)) => user,
                Ok(None) => {
                    let response = json!({
                        "success": false,
                        "message": "Invalid phone number or password",
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
            }
        } else {
            let response = json!({
                "success": false,
                "message": "Either email or phoneNo must be provided",
                "data": null
            });
            return (StatusCode::BAD_REQUEST, Json(response));
        }
    } else if let Some(ref phone) = payload.phone_no {
        match app_state.user_service.find_by_phone(phone).await {
            Ok(Some(user)) => user,
            Ok(None) => {
                let response = json!({
                    "success": false,
                    "message": "Invalid phone number or password",
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
        }
    } else {
        let response = json!({
            "success": false,
            "message": "Either email or phoneNo must be provided",
            "data": null
        });
        return (StatusCode::BAD_REQUEST, Json(response));
    };

    // 2. Verify password
    let is_valid = verify_password(&user.password, &payload.password);
    if !is_valid {
        let response = json!({
            "success": false,
            "message": "Invalid email or password",
            "data": null
        });
        return (StatusCode::UNAUTHORIZED, Json(response));
    }

    // Check if user is active (allow PENDING_KYC for associates)
    if user.status != "active" && user.status != "PENDING_KYC" {
        let response = json!({
            "success": false,
            "message": format!("Account is not active (Status: {})", user.status),
            "data": null
        });
        return (StatusCode::UNAUTHORIZED, Json(response));
    }

    // 3. Generate JWT token
    let token = match create_jwt(&user.id.to_string(), &app_state.jwt_secret, 360) {
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

    // 4. Return response with user_role, associate_type, is_phone_verified
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
                "associate_type": user.associate_type,
                "is_phone_verified": user.is_phone_verified,
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
pub async fn signout(State(_app_state): State<AppState>) -> impl IntoResponse {
    let response = json!({
        "success": true,
        "message": "User signed out successfully",
        "data": null
    });

    (StatusCode::OK, Json(response))
}

/// Send OTP to phone number
#[utoipa::path(
    post,
    path = "/api/auth/send-otp",
    request_body = SendOtpRequest,
    responses(
        (status = 200, description = "OTP sent successfully"),
        (status = 400, description = "Bad request")
    ),
    tag = "Authentication"
)]
pub async fn send_otp(
    State(app_state): State<AppState>,
    ExtractJson(payload): ExtractJson<SendOtpRequest>,
) -> impl IntoResponse {
    let otp = generate_otp();

    // --- DEVELOPER FALLBACK ---
    println!("========================================");
    println!("🔑 [DEV MODE] SEND-OTP INTERCEPT:");
    println!("Phone: {}", payload.phone_no);
    println!("OTP Code: {}", otp);
    println!("========================================");

    // Store OTP in database (10 minute expiry)
    if let Err(e) = app_state
        .user_service
        .store_otp(&payload.phone_no, &otp, 10)
        .await
    {
        let response = json!({
            "success": false,
            "message": format!("Failed to store OTP: {}", e),
            "data": null
        });
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
    }

    // Send SMS
    if let Err(e) = send_sms_otp(&payload.phone_no, &otp).await {
        eprintln!(
            "Warning: Failed to send SMS OTP to {}: {:?}",
            payload.phone_no, e
        );
        // Don't return error in dev mode — OTP is printed to console
    } else {
        println!("✓ OTP SMS sent to {}", payload.phone_no);
    }

    let response = json!({
        "success": true,
        "message": "OTP sent successfully",
        "data": null
    });
    (StatusCode::OK, Json(response))
}

/// Verify OTP and mark phone as verified
#[utoipa::path(
    post,
    path = "/api/auth/verify-otp",
    request_body = VerifyOtpRequest,
    responses(
        (status = 200, description = "Phone verified successfully"),
        (status = 401, description = "Invalid or expired OTP")
    ),
    tag = "Authentication"
)]
pub async fn verify_otp(
    State(app_state): State<AppState>,
    ExtractJson(payload): ExtractJson<VerifyOtpRequest>,
) -> impl IntoResponse {
    // 1. Verify OTP from database
    let otp_valid = match app_state
        .user_service
        .verify_and_consume_otp(&payload.phone_no, &payload.otp)
        .await
    {
        Ok(valid) => valid,
        Err(e) => {
            // "OTP has expired" or "No OTP found"
            let response = json!({
                "success": false,
                "message": e,
                "data": null
            });
            return (StatusCode::UNAUTHORIZED, Json(response));
        }
    };

    if !otp_valid {
        let response = json!({
            "success": false,
            "message": "Invalid OTP",
            "data": null
        });
        return (StatusCode::UNAUTHORIZED, Json(response));
    }

    // 2. Find user by phone and mark as phone-verified
    let user = match app_state
        .user_service
        .find_by_phone(&payload.phone_no)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            let response = json!({
                "success": false,
                "message": "User not found for this phone number",
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
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
        }
    };

    // 3. Set is_phone_verified = true
    if let Err(e) = app_state
        .user_service
        .set_phone_verified(&user.id.to_string())
        .await
    {
        let response = json!({
            "success": false,
            "message": format!("Failed to update verification status: {}", e),
            "data": null
        });
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
    }

    // 4. Generate JWT token
    let token = match create_jwt(&user.id.to_string(), &app_state.jwt_secret, 360) {
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

    // 5. Return authenticated user
    let response = json!({
        "success": true,
        "message": "Phone verified successfully",
        "data": {
            "token": token,
            "user": {
                "id": user.id,
                "first_name": user.first_name,
                "last_name": user.last_name,
                "email": user.email,
                "phone_no": user.phone_no,
                "user_role": user.user_role,
                "associate_type": user.associate_type,
                "is_phone_verified": true,
                "verified": user.verified,
                "status": user.status,
                "created_at": user.created_at
            }
        }
    });

    (StatusCode::OK, Json(response))
}

/// Resend OTP — invalidates old OTP and sends a new one
#[utoipa::path(
    post,
    path = "/api/auth/resend-otp",
    request_body = SendOtpRequest,
    responses(
        (status = 200, description = "OTP resent successfully"),
        (status = 400, description = "Bad request")
    ),
    tag = "Authentication"
)]
pub async fn resend_otp(
    State(app_state): State<AppState>,
    ExtractJson(payload): ExtractJson<SendOtpRequest>,
) -> impl IntoResponse {
    // 1. Invalidate existing OTPs
    let _ = app_state
        .user_service
        .invalidate_otps(&payload.phone_no)
        .await;

    // 2. Generate and store new OTP
    let otp = generate_otp();

    // --- DEVELOPER FALLBACK ---
    println!("========================================");
    println!("🔑 [DEV MODE] RESEND-OTP INTERCEPT:");
    println!("Phone: {}", payload.phone_no);
    println!("OTP Code: {}", otp);
    println!("========================================");

    if let Err(e) = app_state
        .user_service
        .store_otp(&payload.phone_no, &otp, 10)
        .await
    {
        let response = json!({
            "success": false,
            "message": format!("Failed to store OTP: {}", e),
            "data": null
        });
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
    }

    // 3. Send SMS
    if let Err(e) = send_sms_otp(&payload.phone_no, &otp).await {
        eprintln!(
            "Warning: Failed to resend SMS OTP to {}: {:?}",
            payload.phone_no, e
        );
    } else {
        println!("✓ Resend OTP SMS sent to {}", payload.phone_no);
    }

    let response = json!({
        "success": true,
        "message": "OTP resent successfully",
        "data": null
    });
    (StatusCode::OK, Json(response))
}

/// Update associate type (JWT-protected)
#[utoipa::path(
    patch,
    path = "/api/auth/associate-type",
    request_body = UpdateAssociateTypeRequest,
    responses(
        (status = 200, description = "Associate type updated"),
        (status = 403, description = "User is not an associate"),
        (status = 422, description = "Invalid associate type")
    ),
    tag = "Authentication"
)]
pub async fn update_associate_type(
    State(app_state): State<AppState>,
    auth_user: AuthenticationUser,
    ExtractJson(payload): ExtractJson<UpdateAssociateTypeRequest>,
) -> impl IntoResponse {
    // 1. Look up the user from DB to check their role
    let user = match app_state
        .user_service
        .user_repository
        .find_by_id(&auth_user.user_id)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            let response = json!({
                "success": false,
                "message": "User not found",
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
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
        }
    };

    // 2. Verify user_role = "associate"
    if user.user_role != "associate" {
        let response = json!({
            "success": false,
            "message": "Only associate users can set their associate type",
            "data": null
        });
        return (StatusCode::FORBIDDEN, Json(response));
    }

    // 3. Validate associate_type value
    let valid_types = ["broker", "carecrew"];
    if !valid_types.contains(&payload.associate_type.as_str()) {
        let response = json!({
            "success": false,
            "message": "Invalid associate type. Must be 'broker' or 'carecrew'",
            "data": null
        });
        return (StatusCode::UNPROCESSABLE_ENTITY, Json(response));
    }

    // 4. Update in DB
    if let Err(e) = app_state
        .user_service
        .update_associate_type(&auth_user.user_id, &payload.associate_type)
        .await
    {
        let response = json!({
            "success": false,
            "message": format!("Failed to update associate type: {}", e),
            "data": null
        });
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
    }

    let response = json!({
        "success": true,
        "message": "Associate type updated successfully",
        "data": {
            "associateType": payload.associate_type
        }
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
) -> impl IntoResponse {
    // Helper: detect if a string looks like a phone number
    let looks_like_phone = |s: &str| -> bool {
        let trimmed = s.trim();
        trimmed.starts_with('+') || trimmed.chars().all(|c| c.is_ascii_digit())
    };

    // 1. Find user by email OR phone (auto-detect phone numbers in email field)
    let user = if let Some(ref email_val) = payload.email {
        if !email_val.is_empty() {
            if looks_like_phone(email_val) {
                // The "email" field actually contains a phone number
                match app_state.user_service.find_by_phone(email_val).await {
                    Ok(Some(user)) => user,
                    Ok(None) => {
                        let response = json!({
                            "success": false,
                            "message": "User not found with this phone number",
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
                }
            } else {
                // It's a real email
                match app_state.user_service.find_by_email(email_val).await {
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
                }
            }
        } else if let Some(ref phone) = payload.phone_no {
            // email is empty, try phone_no field
            match app_state.user_service.find_by_phone(phone).await {
                Ok(Some(user)) => user,
                Ok(None) => {
                    let response = json!({
                        "success": false,
                        "message": "User not found with this phone number",
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
            }
        } else {
            let response = json!({
                "success": false,
                "message": "Either email or phoneNo must be provided",
                "data": null
            });
            return (StatusCode::BAD_REQUEST, Json(response));
        }
    } else if let Some(ref phone) = payload.phone_no {
        match app_state.user_service.find_by_phone(phone).await {
            Ok(Some(user)) => user,
            Ok(None) => {
                let response = json!({
                    "success": false,
                    "message": "User not found with this phone number",
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
        }
    } else {
        let response = json!({
            "success": false,
            "message": "Either email or phoneNo must be provided",
            "data": null
        });
        return (StatusCode::BAD_REQUEST, Json(response));
    };

    // 2. Generate 6-digit numeric OTP for password reset
    let reset_code = generate_otp();

    // --- DEVELOPER FALLBACK ---
    println!("========================================");
    println!("🔑 [DEV MODE] FORGOT PASSWORD OTP INTERCEPT:");
    println!("Email: {}", user.email);
    println!("OTP Code: {}", reset_code);
    println!("========================================");

    // 3. Store reset code in database
    if let Err(e) = app_state
        .user_service
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

    // 4. Send reset code via email
    if let Err(e) = send_email_otp(&user.email, &reset_code).await {
        eprintln!(
            "Warning: Failed to send password reset email to {}: {:?}",
            user.email, e
        );
    } else {
        println!("✓ Password reset OTP sent to email {}", user.email);
    }

    // 5. Also send reset code via SMS to phone
    if !user.phone_no.is_empty() {
        if let Err(e) = send_sms_otp(&user.phone_no, &reset_code).await {
            eprintln!(
                "Warning: Failed to send password reset SMS to {}: {:?}",
                user.phone_no, e
            );
        } else {
            println!("✓ Password reset OTP sent to phone {}", user.phone_no);
        }
    }

    // 6. Return response
    let response = json!({
        "success": true,
        "message": "Reset code generated and sent to email and phone",
        "data": {
            "email": user.email,
            "phone_no": user.phone_no,
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
) -> impl IntoResponse {
    // 1. Verify reset code
    let user_id = match app_state
        .user_service
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
    if let Err(e) = app_state
        .user_service
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
    let _ = app_state
        .user_service
        .delete_reset_code(&payload.code)
        .await;

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

/// Test OTP Delivery (Manual Trigger for Devs)
/// Usage: POST /api/auth/test-otp
/// Body: { "channel": "sms", "destination": "+919876543210" }
/// Body: { "channel": "email", "destination": "test@example.com" }
#[utoipa::path(
    post,
    path = "/api/auth/test-otp",
    request_body = TestOtpRequest,
    responses(
        (status = 200, description = "OTP Sent", body = ApiResponse<serde_json::Value>),
        (status = 400, description = "Bad Request")
    ),
    tag = "Authentication"
)]
#[allow(dead_code)]
pub async fn test_otp_delivery(
    ExtractJson(payload): ExtractJson<TestOtpRequest>,
) -> impl IntoResponse {
    let otp = generate_otp();

    // --- DEVELOPER FALLBACK ---
    println!("========================================");
    println!("🔑 [DEV MODE] TEST DELIVERY OTP INTERCEPT:");
    println!(
        "Channel: {} | Destination: {}",
        payload.channel, payload.destination
    );
    println!("OTP Code: {}", otp);
    println!("========================================");

    let result = match payload.channel.as_str() {
        "sms" => send_sms_otp(&payload.destination, &otp).await,
        "email" => send_email_otp(&payload.destination, &otp).await,
        _ => {
            let response = json!({
                "success": false,
                "message": "Invalid channel. Use 'sms' or 'email'"
            });
            return (StatusCode::BAD_REQUEST, Json(response)).into_response();
        }
    };

    match result {
        Ok(_) => {
            let response = json!({
                "success": true,
                "message": format!("Test OTP sent to {} via {}", payload.destination, payload.channel),
                "otp": otp  // Only for testing! Remove in production
            });
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response = json!({
                "success": true, // Returning true to unblock frontend testing
                "message": format!("Delivery failed natively due to AWS ({}). OTP was generated and printed in server logs.", e),
                "otp": otp
            });
            (StatusCode::OK, Json(response)).into_response()
        }
    }
}

/// Change password (JWT-protected)
/// The logged-in user provides current_password + new_password.
#[utoipa::path(
    post,
    path = "/api/auth/change-password",
    request_body = ChangePasswordRequest,
    responses(
        (status = 200, description = "Password changed successfully"),
        (status = 401, description = "Current password is incorrect"),
        (status = 400, description = "Bad request")
    ),
    tag = "Authentication"
)]
pub async fn change_password(
    State(app_state): State<AppState>,
    auth_user: AuthenticationUser,
    ExtractJson(payload): ExtractJson<ChangePasswordRequest>,
) -> impl IntoResponse {
    // 1. Find user
    let user = match app_state
        .user_service
        .user_repository
        .find_by_id(&auth_user.user_id)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            let response = json!({
                "success": false,
                "message": "User not found",
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
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
        }
    };

    // 2. Verify current password
    if !verify_password(&user.password, &payload.current_password) {
        let response = json!({
            "success": false,
            "message": "Current password is incorrect",
            "data": null
        });
        return (StatusCode::UNAUTHORIZED, Json(response));
    }

    // 3. Hash and update new password
    let hashed_new = hash_string(&payload.new_password);
    if let Err(e) = app_state
        .user_service
        .update_password(&auth_user.user_id, &hashed_new)
        .await
    {
        let response = json!({
            "success": false,
            "message": format!("Failed to update password: {}", e),
            "data": null
        });
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
    }

    let response = json!({
        "success": true,
        "message": "Password changed successfully",
        "data": {
            "password_updated": true
        }
    });

    (StatusCode::OK, Json(response))
}
