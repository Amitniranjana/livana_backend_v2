use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::admin_logs::{AdminLogsQuery, AdminLogsListResponse, AdminLogsData, AdminLogResponseItem},
};

pub async fn get_admin_logs(
    State(app_state): State<AppState>,
    Query(q): Query<AdminLogsQuery>,
) -> impl axum::response::IntoResponse {
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);

    let mut query = sqlx::QueryBuilder::new(
        "SELECT l.id, l.admin_id, l.action_type, l.target_type, l.target_id, l.details, l.created_at \
         FROM admin_action_logs l \
         WHERE 1=1"
    );

    let mut count_query = sqlx::QueryBuilder::new(
        "SELECT COUNT(*) FROM admin_action_logs l WHERE 1=1"
    );

    if let Some(admin_id) = &q.admin_id {
        query.push(" AND l.admin_id = ");
        query.push_bind(admin_id);
        count_query.push(" AND l.admin_id = ");
        count_query.push_bind(admin_id);
    }
    if let Some(action_type) = &q.action_type {
        query.push(" AND l.action_type = ");
        query.push_bind(action_type);
        count_query.push(" AND l.action_type = ");
        count_query.push_bind(action_type);
    }
    if let Some(target_type) = &q.target_type {
        query.push(" AND l.target_type = ");
        query.push_bind(target_type);
        count_query.push(" AND l.target_type = ");
        count_query.push_bind(target_type);
    }
    if let Some(from_date) = q.from_date {
        query.push(" AND l.created_at >= ");
        query.push_bind(from_date);
        count_query.push(" AND l.created_at >= ");
        count_query.push_bind(from_date);
    }
    if let Some(to_date) = q.to_date {
        query.push(" AND l.created_at <= ");
        query.push_bind(to_date);
        count_query.push(" AND l.created_at <= ");
        count_query.push_bind(to_date);
    }

    query.push(" ORDER BY l.created_at DESC LIMIT ");
    query.push_bind(limit);
    query.push(" OFFSET ");
    query.push_bind(offset);

    let total: i64 = match count_query.build_query_scalar().fetch_one(&app_state.db).await {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB count error"}))),
    };

    let rows = match query.build().fetch_all(&app_state.db).await {
        Ok(r) => r,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB error"}))),
    };

    let logs: Vec<AdminLogResponseItem> = rows.into_iter().map(|row| AdminLogResponseItem {
        id: row.get("id"),
        admin_id: row.get("admin_id"),
        admin_name: None,
        action_type: row.get("action_type"),
        target_type: row.get("target_type"),
        target_id: row.try_get("target_id").ok(),
        details: row.try_get("details").ok(),
        created_at: row.get("created_at"),
    }).collect();

    (StatusCode::OK, Json(json!(AdminLogsListResponse {
        success: true,
        data: AdminLogsData { total, logs },
    })))
}

pub async fn get_admin_logs_by_target(
    State(app_state): State<AppState>,
    Path((target_type, target_id)): Path<(String, Uuid)>,
) -> impl axum::response::IntoResponse {
    let rows = match sqlx::query(
        "SELECT id, admin_id, action_type, target_type, target_id, details, created_at \
         FROM admin_action_logs \
         WHERE target_type = $1 AND target_id = $2 \
         ORDER BY created_at DESC"
    )
    .bind(&target_type)
    .bind(target_id)
    .fetch_all(&app_state.db)
    .await {
        Ok(r) => r,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB error"}))),
    };

    let logs: Vec<AdminLogResponseItem> = rows.into_iter().map(|row| AdminLogResponseItem {
        id: row.get("id"),
        admin_id: row.get("admin_id"),
        admin_name: None,
        action_type: row.get("action_type"),
        target_type: row.get("target_type"),
        target_id: row.try_get("target_id").ok(),
        details: row.try_get("details").ok(),
        created_at: row.get("created_at"),
    }).collect();

    (StatusCode::OK, Json(json!(AdminLogsListResponse {
        success: true,
        data: AdminLogsData { total: logs.len() as i64, logs },
    })))
}
