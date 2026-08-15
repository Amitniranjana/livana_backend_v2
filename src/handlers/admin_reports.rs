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
        Pagination, ReporterInfo, UpdateReportStatusRequest, ReportActionRequest,
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

    let base_query = r#"
        WITH unified_reports AS (
            SELECT id, reporter_id, 'PROPERTY' as entity_type, property_id as entity_id, reason, description as comment, status, admin_notes, created_at 
            FROM property_reports
            UNION ALL
            SELECT id, reporter_id, entity_type, entity_id, reason, NULL as comment, status, admin_notes, created_at
            FROM moderation_reports
            UNION ALL
            SELECT id, user_id as reporter_id, 'POST' as entity_type, news_id as entity_id, reason, NULL as comment, status, admin_notes, created_at
            FROM news_reports
        )
        SELECT r.*,
               CONCAT(u.first_name, ' ', u.last_name) as reporter_name, u.email as reporter_email,
               CASE 
                   WHEN r.status IN ('PENDING_REVIEW', 'open') THEN 'open'
                   WHEN r.status IN ('REVIEWED', 'action_taken', 'resolved') THEN 'resolved'
                   WHEN r.status IN ('DISMISSED', 'dismissed') THEN 'dismissed'
                   ELSE r.status
               END as normalized_status
        FROM unified_reports r
        LEFT JOIN users u ON r.reporter_id = u.id
        WHERE 1=1
    "#;

    let mut query = sqlx::QueryBuilder::new(base_query);
    let mut count_query = sqlx::QueryBuilder::new(
        "WITH unified_reports AS (
            SELECT id, 'PROPERTY' as entity_type, status FROM property_reports
            UNION ALL
            SELECT id, entity_type, status FROM moderation_reports
            UNION ALL
            SELECT id, 'POST' as entity_type, status FROM news_reports
        )
        SELECT COUNT(*) FROM unified_reports r WHERE 1=1"
    );

    if let Some(status) = &q.status {
        let status_lower = status.to_lowercase();
        let statuses: Vec<String> = match status_lower.as_str() {
            "open" => vec!["open".to_string(), "PENDING_REVIEW".to_string()],
            "resolved" => vec!["resolved".to_string(), "REVIEWED".to_string(), "action_taken".to_string()],
            "dismissed" => vec!["dismissed".to_string(), "DISMISSED".to_string()],
            _ => vec![status_lower.clone()],
        };
        
        query.push(" AND r.status IN (");
        count_query.push(" AND r.status IN (");
        
        let mut separated = query.separated(", ");
        let mut separated_count = count_query.separated(", ");
        for s in statuses {
            separated.push_bind(s.clone());
            separated_count.push_bind(s);
        }
        
        query.push(") ");
        count_query.push(") ");
    }

    if let Some(entity_type) = &q.entity_type {
        query.push(" AND UPPER(r.entity_type) = UPPER(");
        query.push_bind(entity_type);
        query.push(") ");
        
        count_query.push(" AND UPPER(r.entity_type) = UPPER(");
        count_query.push_bind(entity_type);
        count_query.push(") ");
    }

    if let Some(search) = &q.search {
        let search_pattern = format!("%{}%", search);
        query.push(" AND (r.reason ILIKE ");
        query.push_bind(search_pattern.clone());
        query.push(" OR CONCAT(u.first_name, ' ', u.last_name) ILIKE ");
        query.push_bind(search_pattern);
        query.push(") ");
        // count query doesn't join users for search currently to stay simple, but since search is rare, we can just do a slower count or skip it.
        // For correctness, count_query needs the JOIN if search is used.
    }

    // Fix count_query if search is present
    if q.search.is_some() {
        count_query = sqlx::QueryBuilder::new(
            "WITH unified_reports AS (
                SELECT id, reporter_id, 'PROPERTY' as entity_type, reason, status FROM property_reports
                UNION ALL
                SELECT id, reporter_id, entity_type, reason, status FROM moderation_reports
                UNION ALL
                SELECT id, user_id as reporter_id, 'POST' as entity_type, reason, status FROM news_reports
            )
            SELECT COUNT(*) FROM unified_reports r LEFT JOIN users u ON r.reporter_id = u.id WHERE 1=1"
        );
        // reapply filters
        if let Some(status) = &q.status {
            let status_lower = status.to_lowercase();
            let statuses: Vec<String> = match status_lower.as_str() {
                "open" => vec!["open".to_string(), "PENDING_REVIEW".to_string()],
                "resolved" => vec!["resolved".to_string(), "REVIEWED".to_string(), "action_taken".to_string()],
                "dismissed" => vec!["dismissed".to_string(), "DISMISSED".to_string()],
                _ => vec![status_lower.clone()],
            };
            count_query.push(" AND r.status IN (");
            let mut separated_count = count_query.separated(", ");
            for s in statuses { separated_count.push_bind(s); }
            count_query.push(") ");
        }
        if let Some(entity_type) = &q.entity_type {
            count_query.push(" AND UPPER(r.entity_type) = UPPER(");
            count_query.push_bind(entity_type);
            count_query.push(") ");
        }
        let search_pattern = format!("%{}%", q.search.as_ref().unwrap());
        count_query.push(" AND (r.reason ILIKE ");
        count_query.push_bind(search_pattern.clone());
        count_query.push(" OR CONCAT(u.first_name, ' ', u.last_name) ILIKE ");
        count_query.push_bind(search_pattern);
        count_query.push(") ");
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
            entity_type: row.get::<String, _>("entity_type").to_uppercase(),
            entity_id: row.get("entity_id"),
            entity_snapshot: None, // skip snapshot for list to save DB calls, or load simple ones
            reason: row.get("reason"),
            comment: row.try_get("comment").unwrap_or(None),
            admin_notes: row.try_get("admin_notes").unwrap_or(None),
            status: row.get("normalized_status"),
            created_at: row.get("created_at"),
        });
    }

    let resp = AdminReportsListResponse {
        success: true,
        data: AdminReportsData {
            reports,
            pagination: Pagination { total, limit, offset },
        }
    };

    (StatusCode::OK, Json(json!(resp)))
}

// ---------------------------------------------------------------------------
// Helper to fetch entity snapshot
// ---------------------------------------------------------------------------
async fn fetch_entity_snapshot(db: &sqlx::PgPool, entity_type: &str, entity_id: Uuid) -> Option<serde_json::Value> {
    match entity_type.to_uppercase().as_str() {
        "PROPERTY" => {
            let row = sqlx::query("SELECT p.title, p.user_id, CONCAT(u.first_name, ' ', u.last_name) as owner_name FROM properties p LEFT JOIN users u ON p.user_id = u.id WHERE p.id = $1")
                .bind(entity_id)
                .fetch_optional(db)
                .await.ok().flatten()?;
            Some(json!({
                "title": row.try_get::<String, _>("title").unwrap_or_default(),
                "owner_id": row.try_get::<Uuid, _>("user_id").ok(),
                "owner_name": row.try_get::<String, _>("owner_name").ok(),
            }))
        },
        "USER" => {
            let row = sqlx::query("SELECT first_name, last_name, email FROM users WHERE id = $1")
                .bind(entity_id)
                .fetch_optional(db)
                .await.ok().flatten()?;
            Some(json!({
                "first_name": row.try_get::<String, _>("first_name").unwrap_or_default(),
                "last_name": row.try_get::<String, _>("last_name").unwrap_or_default(),
                "email": row.try_get::<String, _>("email").ok(),
            }))
        },
        "COMMUNITY" => {
            let row = sqlx::query("SELECT name, description FROM communities WHERE id = $1")
                .bind(entity_id)
                .fetch_optional(db)
                .await.ok().flatten()?;
            Some(json!({
                "name": row.try_get::<String, _>("name").unwrap_or_default(),
                "description": row.try_get::<String, _>("description").ok(),
            }))
        },
        "POST" | "NEWS" => {
            let row = sqlx::query("SELECT title, content FROM news_items WHERE id = $1")
                .bind(entity_id)
                .fetch_optional(db)
                .await.ok().flatten()?;
            Some(json!({
                "title": row.try_get::<String, _>("title").unwrap_or_default(),
                "content": row.try_get::<String, _>("content").ok(),
            }))
        },
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// GET /api/admin/reports/{id}
// ---------------------------------------------------------------------------
pub async fn get_admin_report_detail(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl axum::response::IntoResponse {
    let base_query = r#"
        WITH unified_reports AS (
            SELECT id, reporter_id, 'PROPERTY' as entity_type, property_id as entity_id, reason, description as comment, status, admin_notes, created_at 
            FROM property_reports
            UNION ALL
            SELECT id, reporter_id, entity_type, entity_id, reason, NULL as comment, status, admin_notes, created_at
            FROM moderation_reports
            UNION ALL
            SELECT id, user_id as reporter_id, 'POST' as entity_type, news_id as entity_id, reason, NULL as comment, status, admin_notes, created_at
            FROM news_reports
        )
        SELECT r.*,
               CONCAT(u.first_name, ' ', u.last_name) as reporter_name, u.email as reporter_email,
               CASE 
                   WHEN r.status IN ('PENDING_REVIEW', 'open') THEN 'open'
                   WHEN r.status IN ('REVIEWED', 'action_taken', 'resolved') THEN 'resolved'
                   WHEN r.status IN ('DISMISSED', 'dismissed') THEN 'dismissed'
                   ELSE r.status
               END as normalized_status
        FROM unified_reports r
        LEFT JOIN users u ON r.reporter_id = u.id
        WHERE r.id = $1
    "#;

    let row = match sqlx::query(base_query)
        .bind(id)
        .fetch_optional(&app_state.db)
        .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"success":false,"message":"Report not found"}))),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB error"}))),
    };

    let entity_type: String = row.get::<String, _>("entity_type").to_uppercase();
    let entity_id: Uuid = row.get("entity_id");

    let snapshot = fetch_entity_snapshot(&app_state.db, &entity_type, entity_id).await;

    let report_item = AdminReportListItem {
        id: row.get("id"),
        reporter_user: ReporterInfo {
            id: row.get("reporter_id"),
            name: row.try_get("reporter_name").unwrap_or_default(),
            email: row.try_get("reporter_email").unwrap_or(None),
        },
        entity_type: entity_type.clone(),
        entity_id,
        entity_snapshot: snapshot,
        reason: row.get("reason"),
        comment: row.try_get("comment").unwrap_or(None),
        admin_notes: row.try_get("admin_notes").unwrap_or(None),
        status: row.get("normalized_status"),
        created_at: row.get("created_at"),
    };

    // Fetch history for this entity
    let history_query = r#"
        WITH unified_reports AS (
            SELECT id, 'PROPERTY' as entity_type, property_id as entity_id, reason, status, created_at FROM property_reports
            UNION ALL
            SELECT id, entity_type, entity_id, reason, status, created_at FROM moderation_reports
            UNION ALL
            SELECT id, 'POST' as entity_type, news_id as entity_id, reason, status, created_at FROM news_reports
        )
        SELECT id, reason, 
               CASE 
                   WHEN status IN ('PENDING_REVIEW', 'open') THEN 'open'
                   WHEN status IN ('REVIEWED', 'action_taken', 'resolved') THEN 'resolved'
                   WHEN status IN ('DISMISSED', 'dismissed') THEN 'dismissed'
                   ELSE status
               END as status,
               created_at
        FROM unified_reports
        WHERE UPPER(entity_type) = UPPER($1) AND entity_id = $2
        ORDER BY created_at DESC
    "#;

    let history_rows = match sqlx::query(history_query)
        .bind(&entity_type)
        .bind(entity_id)
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
// PATCH /api/admin/reports/{id}
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

    let mut updated = false;

    // We don't know which table it is in, so we update all three. UUID is globally unique.
    
    // property_reports
    let res1 = sqlx::query("UPDATE property_reports SET status = $1, admin_notes = COALESCE($2, admin_notes) WHERE id = $3")
        .bind(&payload.status)
        .bind(&payload.admin_notes)
        .bind(id)
        .execute(&mut *tx)
        .await;
    if let Ok(r) = res1 { if r.rows_affected() > 0 { updated = true; } }

    // moderation_reports
    let res2 = sqlx::query("UPDATE moderation_reports SET status = $1, admin_notes = COALESCE($2, admin_notes), updated_at = NOW() WHERE id = $3")
        .bind(&payload.status)
        .bind(&payload.admin_notes)
        .bind(id)
        .execute(&mut *tx)
        .await;
    if let Ok(r) = res2 { if r.rows_affected() > 0 { updated = true; } }

    // news_reports
    let res3 = sqlx::query("UPDATE news_reports SET status = $1, admin_notes = COALESCE($2, admin_notes), updated_at = NOW() WHERE id = $3")
        .bind(&payload.status)
        .bind(&payload.admin_notes)
        .bind(id)
        .execute(&mut *tx)
        .await;
    if let Ok(r) = res3 { if r.rows_affected() > 0 { updated = true; } }

    if !updated {
        return (StatusCode::NOT_FOUND, Json(json!({"success":false,"message":"Report not found"})));
    }

    if let Err(_) = tx.commit().await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB commit error"})));
    }

    let _ = log_admin_action(
        &app_state.db,
        &admin_claims.sub,
        "report_updated",
        "report",
        Some(id),
        Some(json!({ "new_status": payload.status, "admin_notes": payload.admin_notes }))
    ).await;

    (StatusCode::OK, Json(json!({"success":true,"message":"Report updated"})))
}

// ---------------------------------------------------------------------------
// DELETE /api/admin/reports/{id}/force
// ---------------------------------------------------------------------------
pub async fn delete_report(
    State(app_state): State<AppState>,
    axum::extract::Extension(admin_claims): axum::extract::Extension<AdminClaims>,
    Path(id): Path<Uuid>,
) -> impl axum::response::IntoResponse {
    let mut tx = match app_state.db.begin().await {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB error"}))),
    };

    let mut deleted = false;
    
    let res1 = sqlx::query("DELETE FROM property_reports WHERE id = $1").bind(id).execute(&mut *tx).await;
    if let Ok(r) = res1 { if r.rows_affected() > 0 { deleted = true; } }

    let res2 = sqlx::query("DELETE FROM moderation_reports WHERE id = $1").bind(id).execute(&mut *tx).await;
    if let Ok(r) = res2 { if r.rows_affected() > 0 { deleted = true; } }

    let res3 = sqlx::query("DELETE FROM news_reports WHERE id = $1").bind(id).execute(&mut *tx).await;
    if let Ok(r) = res3 { if r.rows_affected() > 0 { deleted = true; } }

    if !deleted {
        return (StatusCode::NOT_FOUND, Json(json!({"success":false,"message":"Report not found"})));
    }

    if let Err(_) = tx.commit().await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB commit error"})));
    }

    let _ = log_admin_action(
        &app_state.db,
        &admin_claims.sub,
        "report_deleted",
        "report",
        Some(id),
        None
    ).await;

    (StatusCode::OK, Json(json!({"success":true,"message":"Report deleted successfully"})))
}

// ---------------------------------------------------------------------------
// POST /api/admin/reports/{id}/action
// ---------------------------------------------------------------------------
pub async fn execute_report_action(
    State(app_state): State<AppState>,
    axum::extract::Extension(admin_claims): axum::extract::Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ReportActionRequest>,
) -> impl axum::response::IntoResponse {
    // 1. Find the report to get entity type and id
    let base_query = r#"
        WITH unified_reports AS (
            SELECT id, 'PROPERTY' as entity_type, property_id as entity_id, status FROM property_reports
            UNION ALL
            SELECT id, entity_type, entity_id, status FROM moderation_reports
            UNION ALL
            SELECT id, 'POST' as entity_type, news_id as entity_id, status FROM news_reports
        )
        SELECT entity_type, entity_id FROM unified_reports WHERE id = $1
    "#;

    let row = match sqlx::query(base_query)
        .bind(id)
        .fetch_optional(&app_state.db)
        .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"success":false,"message":"Report not found"}))),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB error"}))),
    };

    let entity_type: String = row.get::<String, _>("entity_type").to_uppercase();
    let entity_id: Uuid = row.get("entity_id");

    let mut tx = match app_state.db.begin().await {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB error"}))),
    };

    // 2. Execute action
    match payload.action.as_str() {
        "suspend-user" => {
            if entity_type == "USER" {
                let _ = sqlx::query("UPDATE users SET account_status = 'suspended' WHERE id = $1").bind(entity_id).execute(&mut *tx).await;
            } else if entity_type == "PROPERTY" {
                // Suspend the owner of the property
                let _ = sqlx::query("UPDATE users SET account_status = 'suspended' WHERE id = (SELECT user_id FROM properties WHERE id = $1)").bind(entity_id).execute(&mut *tx).await;
            }
        },
        "delete-property" | "delist-property" => {
            if entity_type == "PROPERTY" {
                let _ = sqlx::query("UPDATE properties SET status = 'deleted' WHERE id = $1").bind(entity_id).execute(&mut *tx).await;
            }
        },
        "delete-news" | "delete-post" => {
            if entity_type == "POST" || entity_type == "NEWS" {
                let _ = sqlx::query("UPDATE news_items SET status = 'rejected' WHERE id = $1").bind(entity_id).execute(&mut *tx).await;
            } else if entity_type == "COMMUNITY" {
                let _ = sqlx::query("UPDATE communities SET is_active = false WHERE id = $1").bind(entity_id).execute(&mut *tx).await;
            }
        },
        "dismiss" => {
            // just dismiss the report
        },
        _ => return (StatusCode::BAD_REQUEST, Json(json!({"success":false,"message":"Invalid action"}))),
    }

    // 3. Update report status to resolved/dismissed
    let final_status = if payload.action == "dismiss" { "dismissed" } else { "resolved" };
    
    let _ = sqlx::query("UPDATE property_reports SET status = $1 WHERE id = $2").bind(final_status).bind(id).execute(&mut *tx).await;
    let _ = sqlx::query("UPDATE moderation_reports SET status = $1, updated_at = NOW() WHERE id = $2").bind(final_status).bind(id).execute(&mut *tx).await;
    let _ = sqlx::query("UPDATE news_reports SET status = $1, updated_at = NOW() WHERE id = $2").bind(final_status).bind(id).execute(&mut *tx).await;

    if let Err(_) = tx.commit().await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB commit error"})));
    }

    let _ = log_admin_action(
        &app_state.db,
        &admin_claims.sub,
        &format!("report_action_{}", payload.action),
        "report",
        Some(id),
        Some(json!({ "entity_type": entity_type, "entity_id": entity_id }))
    ).await;

    (StatusCode::OK, Json(json!({"success":true,"message":"Action executed and report resolved"})))
}
