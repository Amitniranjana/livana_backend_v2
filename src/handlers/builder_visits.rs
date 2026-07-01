use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::models::visit::SiteVisitRow;
use crate::utils::auth_extractor::AuthenticationUser;

#[derive(Deserialize)]
pub struct BuilderVisitsQuery {
    pub status: Option<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// 5.1 GET /api/builder/visits
pub async fn get_builder_visits(
    auth: AuthenticationUser,
    State(app_state): State<AppState>,
    Query(params): Query<BuilderVisitsQuery>,
) -> impl IntoResponse {
    let user_id = match Uuid::parse_str(&auth.user_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"success": false, "message": "Invalid token"})),
            )
                .into_response();
        }
    };

    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    // We join site_visits with properties AND builder_projects to find visits that belong
    // to either a property owned by the builder OR a project owned by the builder.
    // However, the `provider_id` in site_visits is usually the person who receives the request (the owner).
    // Let's just query where provider_id = user_id.

    let mut query = r#"
        SELECT
            sv.id                                       AS visit_id,
            sv.property_id,
            sv.project_id,
            COALESCE(p.title, bp.project_name)          AS property_title,
            COALESCE(COALESCE(p.locality, p.city), COALESCE(bp.locality, bp.city)) AS property_location,
            sv.user_id,
            sv.provider_id,
            (u.first_name || ' ' || u.last_name)        AS provider_name,
            u.profile_picture                           AS provider_image,
            sv.scheduled_date_time,
            sv.status,
            sv.contact_number,
            sv.notes,
            sv.cancellation_reason,
            sv.created_at
        FROM site_visits sv
        LEFT JOIN properties p ON p.id = sv.property_id
        LEFT JOIN builder_projects bp ON bp.id = sv.project_id
        LEFT JOIN users u ON u.id = sv.user_id
        WHERE sv.provider_id = $1
    "#.to_string();

    if let Some(ref status) = params.status {
        query.push_str(&format!(" AND sv.status = '{}'", status));
    }
    
    if let Some(ref from) = params.from_date {
        query.push_str(&format!(" AND sv.scheduled_date_time >= '{}'", from));
    }
    
    if let Some(ref to) = params.to_date {
        query.push_str(&format!(" AND sv.scheduled_date_time <= '{}'", to));
    }
    
    query.push_str(" ORDER BY sv.scheduled_date_time DESC LIMIT $2 OFFSET $3");

    let rows = sqlx::query_as::<_, SiteVisitRow>(&query)
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&app_state.db)
        .await;

    match rows {
        Ok(visits) => {
            let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM site_visits WHERE provider_id = $1")
                .bind(user_id)
                .fetch_one(&app_state.db)
                .await
                .unwrap_or(0);
                
            // Convert to VisitItem (which is done via mapping or just returning raw rows if acceptable)
            // To perfectly match, we can just return the rows.
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "data": {
                        "visits": visits,
                        "pagination": { "total": total, "limit": limit, "offset": offset }
                    }
                }))
            ).into_response()
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": format!("Error: {}", e)}))
        ).into_response()
    }
}
