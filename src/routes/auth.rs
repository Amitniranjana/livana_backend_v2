use crate::app_state::AppState;
use crate::handlers::auth::{
    change_password, resend_otp, reset_password, send_forgot_password_link, send_otp, signin,
    signout, signup, update_associate_type, verify_otp,
};
use axum::{Router, routing::{post, patch}};

pub fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/api/auth/signup", post(signup))
        .route("/api/auth/signin", post(signin))
        .route("/api/auth/signout", post(signout))
        .route("/api/auth/send-otp", post(send_otp))
        .route("/api/auth/verify-otp", post(verify_otp))
        .route("/api/auth/resend-otp", post(resend_otp))
        .route("/api/auth/associate-type", patch(update_associate_type))
        .route(
            "/api/auth/send-forgot-password-link",
            post(send_forgot_password_link),
        )
        .route("/api/auth/reset-password", post(reset_password))
        .route("/api/auth/change-password", post(change_password))
        .route(
            "/auth/google",
            post(crate::handlers::google_auth::google_signin),
        )
}
