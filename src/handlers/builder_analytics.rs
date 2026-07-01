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
