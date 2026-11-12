//! Issue 110 & ZD v2 Spec — Pending Actions Handlers
//!
//! Handles:
//! - Employer verification (send OTP, verify OTP, admin HR verification)
//! - Co-applicant (invite, remove, me invites, accept with bureau check, admin view)
//! - Landlord reference (tenant submit, admin verify)

use axum::{
    extract::{Extension, Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    handlers::{admin_auth::AdminClaims, zero_deposit::require_auth},
    services::zd_actions_service::{self, ActionError, CoApplicantAcceptReq, CoApplicantInviteReq, LandlordReferenceReq},
    utils::admin_logger::log_admin_action,
};

#[derive(Debug, Deserialize)]
pub struct SendOtpReq {
    pub official_email: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyOtpReq {
    pub otp: String,
}

#[derive(Debug, Deserialize)]
pub struct AdminEmployerVerifReq {
    pub status: String,
    pub method: String,
    pub employer_category: Option<String>,
    pub remarks: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AdminLandlordRefReq {
    pub status: String,
    pub remarks: Option<String>,
}

// ---------------------------------------------------------------------------
// Employer Verification
// ---------------------------------------------------------------------------

pub async fn send_employer_otp(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(zero_deposit_id): Path<Uuid>,
    Json(payload): Json<SendOtpReq>,
) -> impl IntoResponse {
    let user_id = match require_auth(&headers, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => return e.into_response(),
    };

    match zd_actions_service::send_employer_otp(&state.db, zero_deposit_id, user_id, &payload.official_email).await {
        Ok(otp) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "OTP sent to official email successfully",
                "data": { "validity_minutes": 10, "dev_otp": if cfg!(debug_assertions) { Some(otp) } else { None } }
            })),
        )
            .into_response(),
        Err(ActionError::PersonalEmailNotAllowed) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "success": false,
                "error": "PERSONAL_EMAIL_NOT_ALLOWED",
                "message": "Free or personal email domains are not allowed. Please use your official corporate email or request HR verification."
            })),
        )
            .into_response(),
        Err(ActionError::NotOwner) => (
            StatusCode::FORBIDDEN,
            Json(json!({
                "success": false,
                "error": "NOT_OWNER",
                "message": "Not authorized to modify this application"
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn verify_employer_otp(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(zero_deposit_id): Path<Uuid>,
    Json(payload): Json<VerifyOtpReq>,
) -> impl IntoResponse {
    let user_id = match require_auth(&headers, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => return e.into_response(),
    };

    match zd_actions_service::verify_employer_otp(&state.db, zero_deposit_id, user_id, &payload.otp).await {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Employer email verified successfully. Application moved to AI review."
            })),
        )
            .into_response(),
        Err(ActionError::InvalidOtp { remaining_attempts }) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "success": false,
                "error": "INVALID_OTP",
                "message": format!("Invalid OTP. {} attempts remaining.", remaining_attempts)
            })),
        )
            .into_response(),
        Err(ActionError::MaxAttemptsExceeded) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "success": false,
                "error": "MAX_ATTEMPTS_EXCEEDED",
                "message": "Maximum OTP verification attempts exceeded. Verification failed."
            })),
        )
            .into_response(),
        Err(ActionError::OtpExpired) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "success": false,
                "error": "OTP_EXPIRED",
                "message": "OTP has expired. Please request a new OTP."
            })),
        )
            .into_response(),
        Err(ActionError::NotOwner) => (
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": "NOT_OWNER", "message": "Not authorized" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn admin_verify_employer(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path(zero_deposit_id): Path<Uuid>,
    Json(payload): Json<AdminEmployerVerifReq>,
) -> impl IntoResponse {
    match zd_actions_service::admin_verify_employer(
        &state.db,
        &admin_claims.sub,
        zero_deposit_id,
        &payload.status,
        &payload.method,
        payload.employer_category.as_deref(),
        payload.remarks.as_deref(),
    )
    .await
    {
        Ok(()) => {
            let _ = log_admin_action(
                &state.db,
                &admin_claims.sub,
                "ADMIN_VERIFY_EMPLOYER",
                "ZERO_DEPOSIT",
                Some(zero_deposit_id),
                Some(json!({
                    "status": payload.status,
                    "method": payload.method,
                    "employer_category": payload.employer_category,
                    "remarks": payload.remarks
                })),
            )
            .await;

            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": "Employer verification recorded successfully by admin"
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Co-Applicant
// ---------------------------------------------------------------------------

pub async fn invite_co_applicant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(zero_deposit_id): Path<Uuid>,
    Json(payload): Json<CoApplicantInviteReq>,
) -> impl IntoResponse {
    let user_id = match require_auth(&headers, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => return e.into_response(),
    };

    match zd_actions_service::invite_co_applicant(&state.db, zero_deposit_id, user_id, &payload).await {
        Ok(invite_id) => (
            StatusCode::CREATED,
            Json(json!({
                "success": true,
                "message": "Co-applicant invited successfully",
                "data": { "co_applicant_id": invite_id, "window_days": 7 }
            })),
        )
            .into_response(),
        Err(ActionError::CoApplicantAlreadyActive) => (
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "CO_APPLICANT_ALREADY_ACTIVE",
                "message": "An active co-applicant invitation already exists for this application."
            })),
        )
            .into_response(),
        Err(ActionError::NotOwner) => (
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": "NOT_OWNER", "message": "Not authorized" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn remove_co_applicant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((zero_deposit_id, co_applicant_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    let user_id = match require_auth(&headers, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => return e.into_response(),
    };

    match zd_actions_service::remove_co_applicant(&state.db, zero_deposit_id, user_id, co_applicant_id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Co-applicant invite removed. You may now invite another co-applicant."
            })),
        )
            .into_response(),
        Err(ActionError::CannotRemoveAcceptedCoApplicant) => (
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "INVALID_STATE",
                "message": "Cannot remove co-applicant once accepted"
            })),
        )
            .into_response(),
        Err(ActionError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "NOT_FOUND", "message": "Co-applicant not found" })),
        )
            .into_response(),
        Err(ActionError::NotOwner) => (
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": "NOT_OWNER", "message": "Not authorized" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn get_my_co_applicant_invites(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_id = match require_auth(&headers, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => return e.into_response(),
    };

    match zd_actions_service::get_my_co_applicant_invites(&state.db, user_id).await {
        Ok(invites) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "data": invites
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn accept_co_applicant_invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(co_applicant_id): Path<Uuid>,
    Json(payload): Json<CoApplicantAcceptReq>,
) -> impl IntoResponse {
    let _user_id = match require_auth(&headers, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => return e.into_response(),
    };

    match zd_actions_service::accept_co_applicant_invite(&state.db, co_applicant_id, &payload).await {
        Ok(zd_actions_service::CoApplicantOutcome::Accepted { income_considered, cibil_score }) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "status": "accepted",
                "message": "Co-applicant successfully verified and accepted. Application moved to AI review.",
                "data": { "income_considered": income_considered, "cibil_score": cibil_score }
            })),
        )
            .into_response(),
        Ok(zd_actions_service::CoApplicantOutcome::Rejected { reason, cibil_score }) => (
            StatusCode::OK,
            Json(json!({
                "success": false,
                "status": "rejected",
                "message": reason,
                "data": { "cibil_score": cibil_score }
            })),
        )
            .into_response(),
        Ok(zd_actions_service::CoApplicantOutcome::Declined) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "status": "declined",
                "message": "Co-applicant invitation declined."
            })),
        )
            .into_response(),
        Err(ActionError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "NOT_FOUND", "message": "Invite not found" })),
        )
            .into_response(),
        Err(ActionError::InvalidState(msg)) => (
            StatusCode::CONFLICT,
            Json(json!({ "success": false, "error": "INVALID_STATE", "message": msg })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn admin_get_co_applicants(
    State(state): State<AppState>,
    Path(zero_deposit_id): Path<Uuid>,
) -> impl IntoResponse {
    let rows = sqlx::query_scalar::<_, Value>(
        r#"SELECT row_to_json(t) FROM (
            SELECT id, zero_deposit_id, name, relation, mobile, phone, email,
                   employment_type, declared_monthly_income, income_considered,
                   emi_total, cibil_score, status, bureau_summary,
                   esigned_at, invited_at, accepted_at, created_at
            FROM zd_co_applicants
            WHERE zero_deposit_id = $1
            ORDER BY created_at DESC
        ) t"#
    )
    .bind(zero_deposit_id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(data) => (
            StatusCode::OK,
            Json(json!({ "success": true, "data": data })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Landlord Reference
// ---------------------------------------------------------------------------

pub async fn add_landlord_reference(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(zero_deposit_id): Path<Uuid>,
    Json(payload): Json<LandlordReferenceReq>,
) -> impl IntoResponse {
    let user_id = match require_auth(&headers, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => return e.into_response(),
    };

    match zd_actions_service::add_landlord_reference(&state.db, zero_deposit_id, user_id, &payload).await {
        Ok(ref_id) => (
            StatusCode::CREATED,
            Json(json!({
                "success": true,
                "message": "Landlord reference submitted for admin verification",
                "data": { "reference_id": ref_id, "status": "pending" }
            })),
        )
            .into_response(),
        Err(ActionError::NotOwner) => (
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": "NOT_OWNER", "message": "Not authorized" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn admin_verify_landlord_reference(
    State(state): State<AppState>,
    Extension(admin_claims): Extension<AdminClaims>,
    Path((zero_deposit_id, reference_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<AdminLandlordRefReq>,
) -> impl IntoResponse {
    match zd_actions_service::admin_verify_landlord_reference(
        &state.db,
        &admin_claims.sub,
        zero_deposit_id,
        reference_id,
        &payload.status,
        payload.remarks.as_deref(),
    )
    .await
    {
        Ok(()) => {
            let _ = log_admin_action(
                &state.db,
                &admin_claims.sub,
                "ADMIN_VERIFY_LANDLORD_REF",
                "ZD_LANDLORD_REF",
                Some(reference_id),
                Some(json!({
                    "zero_deposit_id": zero_deposit_id,
                    "status": payload.status,
                    "remarks": payload.remarks
                })),
            )
            .await;

            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": "Landlord reference verification recorded",
                    "data": { "status": payload.status }
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

