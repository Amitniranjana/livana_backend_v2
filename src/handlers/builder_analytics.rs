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

    // 1. total_projects (count of distinct project_names for this builder)
    let total_projects: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT project_name) FROM properties WHERE user_id = $1"
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

    // 3. total_visits
    let total_visits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM site_visits sv
         JOIN properties p ON sv.property_id = p.id
         WHERE p.user_id = $1"
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
        total_units: 0, // Mocked pending infrastructure
        units_sold: 0,  // Mocked pending infrastructure
        total_visits,
        total_leads: 0, // Mocked pending infrastructure
        total_views: 0, // Mocked pending infrastructure
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
        JOIN properties p ON sv.property_id = p.id
        WHERE p.user_id = $1
    ");

    if let Some(_proj) = &params.project_id {
        query_str.push_str(" AND p.project_name = $2");
    }

    query_str.push_str(" GROUP BY TO_CHAR(sv.created_at, 'YYYY-MM-DD') ORDER BY date ASC");

    let mut query = sqlx::query(&query_str).bind(user_uuid);
    
    if let Some(proj) = &params.project_id {
        query = query.bind(proj);
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
            COALESCE(p.project_name, 'Unknown') as project_name,
            COUNT(sv.id) as visits
        FROM properties p
        LEFT JOIN site_visits sv ON p.id = sv.property_id
        WHERE p.user_id = $1
        GROUP BY COALESCE(p.project_name, 'Unknown')
    ";

    let mut results = Vec::new();
    if let Ok(rows) = sqlx::query(query).bind(user_uuid).fetch_all(&state.db).await {
        for row in rows {
            let project_name: String = row.get("project_name");
            let visits: i64 = row.get("visits");
            results.push(ProjectPerformanceItem {
                project_id: project_name.clone(),
                project_name,
                views: 0, // Mocked
                visits,
                leads: 0, // Mocked
                units_sold: 0, // Mocked
                units_total: 0, // Mocked
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
