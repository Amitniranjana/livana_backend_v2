use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::dtos::response::ApiResponse;
use crate::utils::auth_extractor::AuthenticationUser;

// -----------------------------------------------------------------------------
// Shared Authorization Helper
// -----------------------------------------------------------------------------
async fn verify_builder_role(state: &AppState, user_id: &str) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let user = state
        .user_service
        .user_repository
        .find_by_id(user_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": e, "data": null})),
            )
        })?;

    if let Some(user) = user {
        if user.user_role != "builder" {
            return Err((
                StatusCode::FORBIDDEN,
                Json(json!({"success": false, "message": "Access denied. Only builders can access this.", "data": null})),
            ));
        }
        Ok(())
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "User not found", "data": null})),
        ))
    }
}

// -----------------------------------------------------------------------------
// API 7.1: Overview
// -----------------------------------------------------------------------------

#[derive(Serialize)]
pub struct DashboardOverview {
    pub total_projects: i64,
    pub active_properties: i64,
    pub total_units: i64,
    pub units_sold: i64,
    pub total_visits: i64,
    pub total_leads: i64,
    pub total_views: i64,
    pub profile_completion_pct: i64,
    pub kyc_status: String,
}

pub async fn get_dashboard_overview(
    State(state): State<AppState>,
    auth_user: AuthenticationUser,
) -> impl IntoResponse {
    if let Err(err) = verify_builder_role(&state, &auth_user.user_id).await {
        return err.into_response();
    }
    
    let user_uuid = Uuid::parse_str(&auth_user.user_id).unwrap_or_default();

    // 1. total_projects
    let total_projects: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM builder_projects WHERE user_id = $1"
    )
    .bind(user_uuid)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    // 2. active_properties
    let active_properties: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM properties WHERE user_id = $1 AND status = 'active'"
    )
    .bind(user_uuid)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    // 3. total_units
    let total_units: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(total_units), 0) FROM builder_projects WHERE user_id = $1"
    )
    .bind(user_uuid)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    // 4. units_sold
    let units_sold: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM properties WHERE project_id IN (SELECT id FROM builder_projects WHERE user_id = $1)"
    )
    .bind(user_uuid)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    // 5. total_visits
    let total_visits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM site_visits sv
         WHERE sv.property_id IN (SELECT id FROM properties WHERE user_id = $1)
            OR sv.project_id IN (SELECT id FROM builder_projects WHERE user_id = $1)"
    )
    .bind(user_uuid)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    // 6. total_leads
    let total_leads: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM project_leads WHERE project_id IN (SELECT id FROM builder_projects WHERE user_id = $1)"
    )
    .bind(user_uuid)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    // 7. total_views
    let total_views: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(views_count), 0) FROM builder_projects WHERE user_id = $1"
    )
    .bind(user_uuid)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);
    
    // KYC Status
    let kyc_status: String = sqlx::query_scalar(
        "SELECT verification_status FROM kyc_submissions WHERE user_id = $1 ORDER BY created_at DESC LIMIT 1"
    )
    .bind(user_uuid)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None)
    .unwrap_or_else(|| "unverified".to_string());

    let overview = DashboardOverview {
        total_projects,
        active_properties,
        total_units,
        units_sold,
        total_visits,
        total_leads,
        total_views,
        profile_completion_pct: 100, // Profile functionality is out of scope for Module 7 Analytics
        kyc_status,
    };

    (
        StatusCode::OK,
        Json(json!(ApiResponse {
            success: true,
            message: "Overview fetched".to_string(),
            data: overview,
        })),
    ).into_response()
}

// -----------------------------------------------------------------------------
// API 7.2: Visits Trend
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct TrendQuery {
    #[allow(dead_code)]
    pub range: Option<String>,
    pub project_id: Option<String>,
}

#[derive(Serialize)]
pub struct VisitTrendItem {
    pub date: String,
    pub visits: i64,
}

pub async fn get_visits_trend(
    State(state): State<AppState>,
    Query(params): Query<TrendQuery>,
    auth_user: AuthenticationUser,
) -> impl IntoResponse {
    if let Err(err) = verify_builder_role(&state, &auth_user.user_id).await {
        return err.into_response();
    }
    
    let user_uuid = Uuid::parse_str(&auth_user.user_id).unwrap_or_default();
    
    // We will group by day (YYYY-MM-DD)
    // Optionally filter by project_name (using project_id param as project_name)
    let mut query_str = String::from("
        SELECT 
            TO_CHAR(sv.created_at, 'YYYY-MM-DD') as date,
            COUNT(*) as visits
        FROM site_visits sv
        LEFT JOIN properties p ON sv.property_id = p.id
        LEFT JOIN builder_projects bp ON sv.project_id = bp.id
        WHERE (p.user_id = $1 OR bp.user_id = $1)
    ");

    if let Some(_proj) = &params.project_id {
        query_str.push_str(" AND (sv.project_id = $2 OR p.project_id = $2)");
    }

    let interval = match params.range.as_deref() {
        Some("7d") => "7 days",
        Some("30d") => "30 days",
        Some("12m") => "12 months",
        _ => "30 days",
    };
    query_str.push_str(&format!(" AND sv.created_at >= NOW() - INTERVAL '{}'", interval));
    
    query_str.push_str(" GROUP BY TO_CHAR(sv.created_at, 'YYYY-MM-DD') ORDER BY date ASC");

    let mut query = sqlx::query(&query_str).bind(user_uuid);
    
    if let Some(proj) = &params.project_id {
        if let Ok(proj_uuid) = Uuid::parse_str(proj) {
            query = query.bind(proj_uuid);
        } else {
            // Invalid UUID passed, bind a dummy one
            query = query.bind(Uuid::nil());
        }
    }

    let mut results = Vec::new();
    if let Ok(rows) = query.fetch_all(&state.db).await {
        for row in rows {
            results.push(VisitTrendItem {
                date: row.get("date"),
                visits: row.get("visits"),
            });
        }
    }

    (
        StatusCode::OK,
        Json(json!(ApiResponse {
            success: true,
            message: "Visits trend fetched".to_string(),
            data: results,
        })),
    ).into_response()
}

// -----------------------------------------------------------------------------
// API 7.5: Project Performance
// -----------------------------------------------------------------------------

#[derive(Serialize)]
pub struct ProjectPerformanceItem {
    pub project_id: String, // Returning project_name as ID since project table doesn't exist
    pub project_name: String,
    pub views: i64,
    pub visits: i64,
    pub leads: i64,
    pub units_sold: i64,
    pub units_total: i64,
}

pub async fn get_project_performance(
    State(state): State<AppState>,
    auth_user: AuthenticationUser,
) -> impl IntoResponse {
    if let Err(err) = verify_builder_role(&state, &auth_user.user_id).await {
        return err.into_response();
    }
    
    let user_uuid = Uuid::parse_str(&auth_user.user_id).unwrap_or_default();

    let query = "
        SELECT 
            bp.id as project_id,
            bp.project_name,
            COALESCE(bp.views_count, 0) as views,
            COALESCE(bp.total_units, 0) as units_total,
            (SELECT COUNT(*) FROM properties p WHERE p.project_id = bp.id) as units_sold,
            (SELECT COUNT(*) FROM site_visits sv WHERE sv.project_id = bp.id) as visits,
            (SELECT COUNT(*) FROM project_leads pl WHERE pl.project_id = bp.id) as leads
        FROM builder_projects bp
        WHERE bp.user_id = $1
    ";

    let mut results = Vec::new();
    if let Ok(rows) = sqlx::query(query).bind(user_uuid).fetch_all(&state.db).await {
        for row in rows {
            let project_id_uuid: Uuid = row.get("project_id");
            results.push(ProjectPerformanceItem {
                project_id: project_id_uuid.to_string(),
                project_name: row.get("project_name"),
                views: row.get::<i64, _>("views"),
                visits: row.get::<i64, _>("visits"),
                leads: row.get::<i64, _>("leads"),
                units_sold: row.get::<i64, _>("units_sold"),
                units_total: row.get::<i32, _>("units_total") as i64,
            });
        }
    }

    (
        StatusCode::OK,
        Json(json!(ApiResponse {
            success: true,
            message: "Project performance fetched".to_string(),
            data: results,
        })),
    ).into_response()
}

// -----------------------------------------------------------------------------
// API 7.8: Top Properties
// -----------------------------------------------------------------------------

#[derive(Serialize)]
pub struct TopPropertyItem {
    pub id: Uuid,
    pub title: String,
    pub visits: i64,
}

pub async fn get_top_properties(
    State(state): State<AppState>,
    auth_user: AuthenticationUser,
) -> impl IntoResponse {
    if let Err(err) = verify_builder_role(&state, &auth_user.user_id).await {
        return err.into_response();
    }
    
    let user_uuid = Uuid::parse_str(&auth_user.user_id).unwrap_or_default();

    let query = "
        SELECT 
            p.id,
            p.title,
            COUNT(sv.id) as visits
        FROM properties p
        LEFT JOIN site_visits sv ON p.id = sv.property_id
        WHERE p.user_id = $1
        GROUP BY p.id, p.title
        ORDER BY visits DESC
        LIMIT 10
    ";

    let mut results = Vec::new();
    if let Ok(rows) = sqlx::query(query).bind(user_uuid).fetch_all(&state.db).await {
        for row in rows {
            results.push(TopPropertyItem {
                id: row.get("id"),
                title: row.get("title"),
                visits: row.get("visits"),
            });
        }
    }

    (
        StatusCode::OK,
        Json(json!(ApiResponse {
            success: true,
            message: "Top properties fetched".to_string(),
            data: results,
        })),
    ).into_response()
}

// -----------------------------------------------------------------------------
// API 7.3: Leads Trend
// -----------------------------------------------------------------------------

#[derive(Serialize)]
pub struct LeadTrendItem {
    pub date: String,
    pub leads: i64,
}

pub async fn get_leads_trend(
    State(state): State<AppState>,
    Query(params): Query<TrendQuery>,
    auth_user: AuthenticationUser,
) -> impl IntoResponse {
    if let Err(err) = verify_builder_role(&state, &auth_user.user_id).await {
        return err.into_response();
    }
    
    let user_uuid = Uuid::parse_str(&auth_user.user_id).unwrap_or_default();
    
    let mut query_str = String::from("
        SELECT 
            TO_CHAR(pl.created_at, 'YYYY-MM-DD') as date,
            COUNT(*) as leads
        FROM project_leads pl
        JOIN builder_projects bp ON pl.project_id = bp.id
        WHERE bp.user_id = $1
    ");

    if let Some(_proj) = &params.project_id {
        query_str.push_str(" AND pl.project_id = $2");
    }

    let interval = match params.range.as_deref() {
        Some("7d") => "7 days",
        Some("30d") => "30 days",
        Some("12m") => "12 months",
        _ => "30 days",
    };
    query_str.push_str(&format!(" AND pl.created_at >= NOW() - INTERVAL '{}'", interval));

    query_str.push_str(" GROUP BY TO_CHAR(pl.created_at, 'YYYY-MM-DD') ORDER BY date ASC");

    let mut query = sqlx::query(&query_str).bind(user_uuid);
    
    if let Some(proj) = &params.project_id {
        if let Ok(proj_uuid) = Uuid::parse_str(proj) {
            query = query.bind(proj_uuid);
        } else {
            query = query.bind(Uuid::nil());
        }
    }

    let mut results = Vec::new();
    if let Ok(rows) = query.fetch_all(&state.db).await {
        for row in rows {
            results.push(LeadTrendItem {
                date: row.get("date"),
                leads: row.get("leads"),
            });
        }
    }

    (
        StatusCode::OK,
        Json(json!(ApiResponse {
            success: true,
            message: "Leads trend fetched".to_string(),
            data: results,
        })),
    ).into_response()
}

// -----------------------------------------------------------------------------
// API 7.4: Views Trend (Synthetic Data as per Plan)
// -----------------------------------------------------------------------------

#[derive(Serialize)]
pub struct ViewTrendItem {
    pub date: String,
    pub views: i64,
}

pub async fn get_views_trend(
    State(state): State<AppState>,
    Query(params): Query<TrendQuery>,
    auth_user: AuthenticationUser,
) -> impl IntoResponse {
    if let Err(err) = verify_builder_role(&state, &auth_user.user_id).await {
        return err.into_response();
    }
    
    let user_uuid = Uuid::parse_str(&auth_user.user_id).unwrap_or_default();
    
    // Get aggregate views
    let mut query_str = String::from("
        SELECT COALESCE(SUM(bp.views_count), 0)
        FROM builder_projects bp
        WHERE bp.user_id = $1
    ");

    if let Some(_proj) = &params.project_id {
        query_str.push_str(" AND bp.id = $2");
    }

    let mut query = sqlx::query_scalar(&query_str).bind(user_uuid);
    if let Some(proj) = &params.project_id {
        if let Ok(proj_uuid) = Uuid::parse_str(proj) {
            query = query.bind(proj_uuid);
        } else {
            query = query.bind(Uuid::nil());
        }
    }

    let total_views: i64 = query.fetch_one(&state.db).await.unwrap_or(0);

    let days = match params.range.as_deref() {
        Some("7d") => 7,
        Some("30d") => 30,
        Some("12m") => 365,
        _ => 30,
    };
    
    // Spread views over days using simple math (as flat views_count doesn't hold history)
    let avg_views = if days > 0 { total_views / days } else { 0 };
    
    let mut results = Vec::new();
    let now = chrono::Utc::now();
    for i in (0..days).rev() {
        let d = now - chrono::Duration::days(i);
        results.push(ViewTrendItem {
            date: d.format("%Y-%m-%d").to_string(),
            views: avg_views,
        });
    }

    (
        StatusCode::OK,
        Json(json!(ApiResponse {
            success: true,
            message: "Views trend fetched".to_string(),
            data: results,
        })),
    ).into_response()
}

// -----------------------------------------------------------------------------
// API 7.6: Leads List (Paginated)
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct LeadsQuery {
    pub project_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize)]
pub struct LeadItem {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub phone: String,
    pub message: Option<String>,
    pub preferred_visit_date: Option<chrono::NaiveDate>,
    pub status: String,
    pub created_at: chrono::NaiveDateTime,
}

pub async fn get_leads(
    State(state): State<AppState>,
    Query(params): Query<LeadsQuery>,
    auth_user: AuthenticationUser,
) -> impl IntoResponse {
    if let Err(err) = verify_builder_role(&state, &auth_user.user_id).await {
        return err.into_response();
    }
    
    let user_uuid = Uuid::parse_str(&auth_user.user_id).unwrap_or_default();
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let offset = params.offset.unwrap_or(0);
    
    let mut query_str = String::from("
        SELECT pl.* 
        FROM project_leads pl
        JOIN builder_projects bp ON pl.project_id = bp.id
        WHERE bp.user_id = $1
    ");

    let mut bind_idx = 2;
    if let Some(_proj) = &params.project_id {
        query_str.push_str(&format!(" AND pl.project_id = ${}", bind_idx));
        bind_idx += 1;
    }
    if let Some(_status) = &params.status {
        query_str.push_str(&format!(" AND pl.status = ${}", bind_idx));
        bind_idx += 1;
    }

    query_str.push_str(&format!(" ORDER BY pl.created_at DESC LIMIT ${} OFFSET ${}", bind_idx, bind_idx + 1));

    let mut query = sqlx::query(&query_str).bind(user_uuid);
    
    if let Some(proj) = &params.project_id {
        if let Ok(proj_uuid) = Uuid::parse_str(proj) {
            query = query.bind(proj_uuid);
        } else {
            query = query.bind(Uuid::nil());
        }
    }
    if let Some(status) = &params.status {
        query = query.bind(status.clone());
    }
    
    query = query.bind(limit).bind(offset);

    let mut results = Vec::new();
    if let Ok(rows) = query.fetch_all(&state.db).await {
        for row in rows {
            results.push(LeadItem {
                id: row.get("id"),
                project_id: row.get("project_id"),
                name: row.get("name"),
                phone: row.get("phone"),
                message: row.try_get("message").unwrap_or(None),
                preferred_visit_date: row.try_get("preferred_visit_date").unwrap_or(None),
                status: row.get("status"),
                created_at: row.get("created_at"),
            });
        }
    }

    (
        StatusCode::OK,
        Json(json!(ApiResponse {
            success: true,
            message: "Leads fetched".to_string(),
            data: results,
        })),
    ).into_response()
}

// -----------------------------------------------------------------------------
// API 7.7: Update Lead Status
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct UpdateLeadStatusRequest {
    pub status: String,
}

pub async fn update_lead_status(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    auth_user: AuthenticationUser,
    Json(payload): Json<UpdateLeadStatusRequest>,
) -> impl IntoResponse {
    if let Err(err) = verify_builder_role(&state, &auth_user.user_id).await {
        return err.into_response();
    }
    
    let user_uuid = Uuid::parse_str(&auth_user.user_id).unwrap_or_default();
    let lead_uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(json!(ApiResponse {
            success: false, message: "Invalid lead ID".to_string(), data: json!(null)
        }))).into_response(),
    };

    // Verify ownership
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM project_leads pl
            JOIN builder_projects bp ON pl.project_id = bp.id
            WHERE pl.id = $1 AND bp.user_id = $2
        )"
    )
    .bind(lead_uuid)
    .bind(user_uuid)
    .fetch_one(&state.db)
    .await
    .unwrap_or(false);

    if !exists {
        return (StatusCode::NOT_FOUND, Json(json!(ApiResponse {
            success: false, message: "Lead not found".to_string(), data: json!(null)
        }))).into_response();
    }

    let update = sqlx::query(
        "UPDATE project_leads SET status = $1, updated_at = NOW() WHERE id = $2"
    )
    .bind(&payload.status)
    .bind(lead_uuid)
    .execute(&state.db)
    .await;

    match update {
        Ok(_) => (StatusCode::OK, Json(json!(ApiResponse {
            success: true,
            message: "Lead status updated".to_string(),
            data: json!({ "id": lead_uuid, "status": payload.status }),
        }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(ApiResponse {
            success: false,
            message: e.to_string(),
            data: json!(null),
        }))).into_response(),
    }
}
