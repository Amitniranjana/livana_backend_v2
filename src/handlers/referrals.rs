use crate::app_state::AppState;
use crate::dtos::response::{ApiResponse, ReferralInfoData};
use crate::utils::auth_extractor::AuthenticationUser;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    response::Json,
};
use serde_json::json;

/// Get referral information for the logged-in user
#[utoipa::path(
    get,
    path = "/api/v1/referrals/me",
    responses(
        (status = 200, description = "Referral info fetched", body = ApiResponse<ReferralInfoData>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "User not found")
    ),
    tag = "Referrals",
    security(("bearer_auth" = []))
)]
pub async fn get_referrals_me(
    State(app_state): State<AppState>,
    auth_user: AuthenticationUser,
) -> impl IntoResponse {
    let user_id = auth_user.user_id;

    // Fetch user to get referral_code
    let user_opt = match app_state.user_service.user_repository.find_by_id(&user_id).await {
        Ok(user) => user,
        Err(e) => {
            let response = json!({
                "success": false,
                "message": format!("Error finding user: {}", e),
                "data": null
            });
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
        }
    };

    let user = match user_opt {
        Some(u) => u,
        None => {
            let response = json!({
                "success": false,
                "message": "User not found",
                "data": null
            });
            return (StatusCode::NOT_FOUND, Json(response));
        }
    };

    // Fetch referral statistics
    let (total_referrals, total_rewards_earned, pending_referrals) = 
        match app_state.user_service.user_repository.get_referral_stats(&user_id).await {
            Ok(stats) => stats,
            Err(_) => (0, 0, 0), // Default if error
        };

    let referral_code = user.referral_code;
    let referral_link = format!("https://yourapp.com/join?ref={}", referral_code);

    let referral_data = ReferralInfoData {
        referral_code,
        referral_link,
        total_referrals,
        total_rewards_earned,
        pending_referrals,
    };

    let response = ApiResponse {
        success: true,
        message: "Referral info fetched".to_string(),
        data: referral_data,
    };

    (StatusCode::OK, Json(json!(response)))
}

/// Get referral rewards for the logged-in user
#[utoipa::path(
    get,
    path = "/api/v1/referrals/rewards",
    responses(
        (status = 200, description = "Referral rewards fetched", body = ApiResponse<crate::dtos::response::ReferralRewardsResponseData>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Referrals",
    security(("bearer_auth" = []))
)]
pub async fn get_referrals_rewards(
    State(app_state): State<AppState>,
    auth_user: AuthenticationUser,
) -> impl IntoResponse {
    let user_id = auth_user.user_id;

    let rewards = match app_state.user_service.user_repository.get_referral_rewards(&user_id).await {
        Ok(rewards) => rewards,
        Err(e) => {
            let response = json!({
                "success": false,
                "message": format!("Error fetching rewards: {}", e),
                "data": null
            });
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
        }
    };

    let total_earned: i32 = rewards.iter().map(|r| r.amount).sum();

    let data = crate::dtos::response::ReferralRewardsResponseData {
        total_earned,
        rewards,
    };

    let response = ApiResponse {
        success: true,
        message: "Referral rewards fetched".to_string(),
        data,
    };

    (StatusCode::OK, Json(json!(response)))
}

/// Get referral history for the logged-in user
#[utoipa::path(
    get,
    path = "/api/v1/referrals/history",
    responses(
        (status = 200, description = "Referral history fetched", body = ApiResponse<crate::dtos::response::ReferralHistoryResponseData>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Referrals",
    security(("bearer_auth" = []))
)]
pub async fn get_referrals_history(
    State(app_state): State<AppState>,
    auth_user: AuthenticationUser,
) -> impl IntoResponse {
    let user_id = auth_user.user_id;

    let referrals = match app_state.user_service.user_repository.get_referral_history(&user_id).await {
        Ok(history) => history,
        Err(e) => {
            let response = json!({
                "success": false,
                "message": format!("Error fetching history: {}", e),
                "data": null
            });
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
        }
    };

    let data = crate::dtos::response::ReferralHistoryResponseData {
        referrals,
    };

    let response = ApiResponse {
        success: true,
        message: "Referral history fetched".to_string(),
        data,
    };

    (StatusCode::OK, Json(json!(response)))
}
