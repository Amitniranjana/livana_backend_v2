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
    dtos::admin_reports::{
        AdminReportDetailData, AdminReportDetailResponse, AdminReportHistoryItem,
        AdminReportListItem, AdminReportsData, AdminReportsListResponse, AdminReportsQuery,
        Pagination, PropertySnapshot, ReporterInfo, UpdateReportStatusRequest,
    },
    handlers::admin_auth::AdminClaims,
    utils::admin_logger::log_admin_action,
};

// ---------------------------------------------------------------------------
// GET /api/admin/reports
// ---------------------------------------------------------------------------
pub async fn get_admin_reports(
    State(app_state): State<AppState>,
    Query(q): Query<AdminReportsQuery>,
) -> impl axum::response::IntoResponse {
    let limit = q.limit.unwrap_or(10).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);

    let mut query = sqlx::QueryBuilder::new(
        "SELECT r.id, r.property_id, r.reporter_id, r.reason, r.description as comment, r.status, r.created_at, \
         u.full_name as reporter_name, u.email as reporter_email, \
         p.title as property_title, p.user_id as owner_id, \
         ou.full_name as owner_name \
         FROM property_reports r \
         LEFT JOIN users u ON r.reporter_id = u.id \
         LEFT JOIN properties p ON r.property_id = p.id \
         LEFT JOIN users ou ON p.user_id = ou.id \
         WHERE 1=1"
    );

    let mut count_query = sqlx::QueryBuilder::new(
        "SELECT COUNT(*) FROM property_reports r WHERE 1=1"
    );

    if let Some(status) = &q.status {
        query.push(" AND r.status = ");
        query.push_bind(status);
        count_query.push(" AND r.status = ");
        count_query.push_bind(status);
    }

    if let Some(prop_id) = q.property_id {
        query.push(" AND r.property_id = ");
        query.push_bind(prop_id);
        count_query.push(" AND r.property_id = ");
        count_query.push_bind(prop_id);
    }

    query.push(" ORDER BY r.created_at DESC LIMIT ");
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

    let mut reports = Vec::new();
    for row in rows {
        reports.push(AdminReportListItem {
            id: row.get("id"),
            reporter_user: ReporterInfo {
                id: row.get("reporter_id"),
                name: row.try_get("reporter_name").unwrap_or_default(),
                email: row.try_get("reporter_email").unwrap_or(None),
            },
            property_id: row.get("property_id"),
            property_snapshot: PropertySnapshot {
                title: row.try_get("property_title").unwrap_or_default(),
                owner_id: row.try_get("owner_id").unwrap_or(Uuid::nil()), // properties should have user_id, but handling gracefully
                owner_name: row.try_get("owner_name").unwrap_or(None),
            },
            reason: row.get("reason"),
            comment: row.try_get("comment").unwrap_or(None),
            status: row.get("status"),
            created_at: row.get("created_at"),
        });
    }

    let resp = AdminReportsListResponse {
        success: true,
        data: AdminReportsData {
            reports,
            pagination: Pagination {
                total,
                limit,
                offset,
            }
        }
    };

    (StatusCode::OK, Json(json!(resp)))
}

// ---------------------------------------------------------------------------
// GET /api/admin/reports/{id}
// ---------------------------------------------------------------------------
pub async fn get_admin_report_detail(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl axum::response::IntoResponse {
    let row = match sqlx::query(
        "SELECT r.id, r.property_id, r.reporter_id, r.reason, r.description as comment, r.status, r.created_at, \
         u.full_name as reporter_name, u.email as reporter_email, \
         p.title as property_title, p.user_id as owner_id, \
         ou.full_name as owner_name \
         FROM property_reports r \
         LEFT JOIN users u ON r.reporter_id = u.id \
         LEFT JOIN properties p ON r.property_id = p.id \
         LEFT JOIN users ou ON p.user_id = ou.id \
         WHERE r.id = $1"
    )
    .bind(id)
    .fetch_optional(&app_state.db)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"success":false,"message":"Report not found"}))),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB error"}))),
    };

    let property_id: Uuid = row.get("property_id");

    let report_item = AdminReportListItem {
        id: row.get("id"),
        reporter_user: ReporterInfo {
            id: row.get("reporter_id"),
            name: row.try_get("reporter_name").unwrap_or_default(),
            email: row.try_get("reporter_email").unwrap_or(None),
        },
        property_id,
        property_snapshot: PropertySnapshot {
            title: row.try_get("property_title").unwrap_or_default(),
            owner_id: row.try_get("owner_id").unwrap_or(Uuid::nil()),
            owner_name: row.try_get("owner_name").unwrap_or(None),
        },
        reason: row.get("reason"),
        comment: row.try_get("comment").unwrap_or(None),
        status: row.get("status"),
        created_at: row.get("created_at"),
    };

    // Fetch history for this property
    let history_rows = match sqlx::query("SELECT id, reason, status, created_at FROM property_reports WHERE property_id = $1 ORDER BY created_at DESC")
        .bind(property_id)
        .fetch_all(&app_state.db)
        .await
    {
        Ok(r) => r,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB error fetching history"}))),
    };

    let report_history = history_rows.into_iter().map(|hr| AdminReportHistoryItem {
        id: hr.get("id"),
        reason: hr.get("reason"),
        status: hr.get("status"),
        created_at: hr.get("created_at"),
    }).collect::<Vec<_>>();

    let resp = AdminReportDetailResponse {
        success: true,
        data: AdminReportDetailData {
            report: report_item,
            report_history,
        }
    };

    (StatusCode::OK, Json(json!(resp)))
}

// ---------------------------------------------------------------------------
// PATCH /api/admin/reports/{id}/status
// ---------------------------------------------------------------------------
pub async fn update_report_status(
    State(app_state): State<AppState>,
    axum::extract::Extension(admin_claims): axum::extract::Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateReportStatusRequest>,
) -> impl axum::response::IntoResponse {
    let mut tx = match app_state.db.begin().await {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB error"}))),
    };

    let report_row = match sqlx::query("SELECT property_id, status FROM property_reports WHERE id = $1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"success":false,"message":"Report not found"}))),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB error"}))),
    };

    let property_id: Uuid = report_row.get("property_id");

    match sqlx::query("UPDATE property_reports SET status = $1 WHERE id = $2")
        .bind(&payload.status)
        .bind(id)
        .execute(&mut *tx)
        .await
    {
        Ok(_) => {},
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB update error"}))),
    };

    if let Some(note) = &payload.resolution_note {
        if note.to_lowercase().contains("delist") || note.to_lowercase().contains("remov") {
            let _ = sqlx::query("UPDATE properties SET status = 'delisted' WHERE id = $1")
                .bind(property_id)
                .execute(&mut *tx)
                .await;
        }
    }

    if let Err(_) = tx.commit().await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB commit error"})));
    }

    let _ = log_admin_action(
        &app_state.db,
        &admin_claims.sub,
        "report_resolved",
        "report",
        Some(id),
        Some(json!({ "new_status": payload.status, "resolution_note": payload.resolution_note }))
    ).await;

    (StatusCode::OK, Json(json!({"success":true,"message":"Report status updated"})))
}
