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
        news::{AdminNewsActionRequest, NewsActionRequest, NewsCreateRequest, NewsUpdateRequest},
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
    _auth: AuthenticationUser,
    Json(req): Json<NewsCreateRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // In a real app, verify `auth` is an admin. We assume authorized here for MVP.
    let news = app_state.news_service.create_news(req).await?;

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
