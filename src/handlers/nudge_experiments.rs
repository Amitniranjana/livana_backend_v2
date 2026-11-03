use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::nudge::{CreateExperimentRequest, SendNudgeRequest},
    services::nudge_service,
};

pub async fn create_experiment(
    State(state): State<AppState>,
    Json(payload): Json<CreateExperimentRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match nudge_service::create_experiment(&state.db, &payload).await {
        Ok(exp) => Ok(Json(json!({ "success": true, "data": exp }))),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "message": e })),
        )),
    }
}

pub async fn get_experiment_results(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match nudge_service::get_experiment_results(&state.db, id).await {
        Ok(results) => Ok(Json(json!({ "success": true, "data": results }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": e })),
        )),
    }
}

pub async fn send_nudge_reminder(
    State(state): State<AppState>,
    Json(payload): Json<SendNudgeRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match nudge_service::send_nudge_for_tenant(
        &state.db,
        payload.tenant_id,
        payload.zero_deposit_id,
        payload.message.as_deref(),
    )
    .await
    {
        Ok(log_id) => Ok(Json(json!({
            "success": true,
            "data": { "log_id": log_id }
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": e })),
        )),
    }
}
