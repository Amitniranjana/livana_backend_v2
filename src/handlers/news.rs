use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::{
        news::{
            AdminNewsActionRequest, NewsActionRequest, NewsCommentRequest, NewsCreateRequest,
            NewsReportRequest, NewsUpdateRequest,
        },
        response::ApiResponse,
    },
    utils::{api_error::ApiError, auth_extractor::AuthenticationUser},
};

#[derive(Deserialize)]
pub struct NewsQuery {
    pub category: Option<String>,
}

pub async fn get_news(
    State(app_state): State<AppState>,
    Query(query): Query<NewsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let news = app_state.news_service.fetch_news(query.category).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            message: "News fetched successfully".to_string(),
            data: news,
        }),
    ))
}

pub async fn create_news(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Json(req): Json<NewsCreateRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Admin creates news - status approved
    let author_id = Uuid::parse_str(&auth.user_id).unwrap_or_default();
    let news = app_state.news_service.create_news(req, author_id, true).await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            message: "News item created".to_string(),
            data: news,
        }),
    ))
}

pub async fn update_news(
    State(app_state): State<AppState>,
    _auth: AuthenticationUser,
    Path(id): Path<Uuid>,
    Json(req): Json<NewsUpdateRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let news = app_state.news_service.update_news(id, req).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            message: "News item updated".to_string(),
            data: news,
        }),
    ))
}

pub async fn admin_action_news(
    State(app_state): State<AppState>,
    _auth: AuthenticationUser,
    Path(id): Path<Uuid>,
    Json(req): Json<AdminNewsActionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let news = app_state.news_service.admin_action(id, req).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            message: "Admin action applied".to_string(),
            data: news,
        }),
    ))
}

pub async fn track_news_action(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<NewsActionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let news = app_state.news_service.track_action(id, req).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            message: "Action tracked successfully".to_string(),
            data: news,
        }),
    ))
}

pub async fn user_create_news(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Json(req): Json<NewsCreateRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // User creates news - status pending
    let author_id = Uuid::parse_str(&auth.user_id).unwrap_or_default();
    let news = app_state.news_service.create_news(req, author_id, false).await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            message: "News submitted for verification".to_string(),
            data: news,
        }),
    ))
}

pub async fn like_news(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id).unwrap_or_default();
    app_state.news_service.like_news(id, user_id).await?;
    Ok((StatusCode::OK, Json(ApiResponse { success: true, message: "Liked".to_string(), data: serde_json::json!({}) })))
}

pub async fn unlike_news(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id).unwrap_or_default();
    app_state.news_service.unlike_news(id, user_id).await?;
    Ok((StatusCode::OK, Json(ApiResponse { success: true, message: "Unliked".to_string(), data: serde_json::json!({}) })))
}

pub async fn save_news(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id).unwrap_or_default();
    app_state.news_service.save_news(id, user_id).await?;
    Ok((StatusCode::OK, Json(ApiResponse { success: true, message: "Saved".to_string(), data: serde_json::json!({}) })))
}

pub async fn unsave_news(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id).unwrap_or_default();
    app_state.news_service.unsave_news(id, user_id).await?;
    Ok((StatusCode::OK, Json(ApiResponse { success: true, message: "Unsaved".to_string(), data: serde_json::json!({}) })))
}

pub async fn report_news(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(id): Path<Uuid>,
    Json(req): Json<NewsReportRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id).unwrap_or_default();
    app_state.news_service.report_news(id, user_id, req).await?;
    Ok((StatusCode::OK, Json(ApiResponse { success: true, message: "Reported".to_string(), data: serde_json::json!({}) })))
}

pub async fn add_comment(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(id): Path<Uuid>,
    Json(req): Json<NewsCommentRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id).unwrap_or_default();
    let comment = app_state.news_service.add_comment(id, user_id, req).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse { success: true, message: "Comment added".to_string(), data: comment })))
}

pub async fn get_comments(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let comments = app_state.news_service.get_comments(id).await?;
    Ok((StatusCode::OK, Json(ApiResponse { success: true, message: "Comments fetched".to_string(), data: comments })))
}
