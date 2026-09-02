use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::dtos::builder_crm::{
    CrmLeadListResponse, CrmLeadPagination, CrmLeadPayload, CrmLeadQuery, CrmLeadResponse,
    CrmLeadStatusUpdate,
};
use crate::dtos::response::ApiResponse;
use crate::utils::auth_extractor::AuthenticationUser;

const VALID_SOURCES: &[&str] = &[
    "99acres", "magicbricks", "housing", "nobroker", "facebook_ads", 
    "google_ads", "referral", "walk_in", "other"
];

const VALID_STATUSES: &[&str] = &[
    "new", "contacted", "site_visit_scheduled", "negotiation", "converted", "lost"
];

const VALID_PRIORITIES: &[&str] = &[
    "hot", "warm", "cold"
];

// -----------------------------------------------------------------------------
// Shared Authorization Helper (mirrors builder_analytics.rs)
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
// Row -> DTO mapping helper (inherent impl on the crate's own type, defined
// here in the handlers module rather than in dtos/builder_crm.rs, so the
// dtos file stays free of sqlx-specific decoding logic)
// -----------------------------------------------------------------------------
fn crm_lead_from_row(row: &sqlx::postgres::PgRow) -> CrmLeadResponse {
    CrmLeadResponse {
        id: row.get("id"),
        user_id: row.get("user_id"),
        project_id: row.try_get("project_id").unwrap_or(None),
        project_name: row.try_get("project_name").unwrap_or(None),
        source: row.get("source"),
        source_detail: row.try_get("source_detail").unwrap_or(None),
        name: row.get("name"),
        phone: row.get("phone"),
        email: row.try_get("email").unwrap_or(None),
        budget_min: row.try_get("budget_min").unwrap_or(None),
        budget_max: row.try_get("budget_max").unwrap_or(None),
        requirement: row.try_get("requirement").unwrap_or(None),
        location_preference: row.try_get("location_preference").unwrap_or(None),
        status: row.get("status"),
        priority: row.get("priority"),
        notes: row.try_get("notes").unwrap_or(None),
        next_follow_up_date: row.try_get("next_follow_up_date").unwrap_or(None),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

// -----------------------------------------------------------------------------
// GET /api/builder/crm-leads
// -----------------------------------------------------------------------------
pub async fn get_crm_leads(
    State(state): State<AppState>,
    Query(params): Query<CrmLeadQuery>,
    auth_user: AuthenticationUser,
) -> impl IntoResponse {
    if let Err(err) = verify_builder_role(&state, &auth_user.user_id).await {
        return err.into_response();
    }

    let user_uuid = Uuid::parse_str(&auth_user.user_id).unwrap_or_default();
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let offset = params.offset.unwrap_or(0).max(0);

    let mut query_str = String::from(
        "SELECT cl.*, bp.project_name AS project_name
         FROM builder_crm_leads cl
         LEFT JOIN builder_projects bp ON cl.project_id = bp.id
         WHERE cl.user_id = $1",
    );

    let mut bind_idx = 2;
    if params.status.is_some() {
        query_str.push_str(&format!(" AND cl.status = ${}", bind_idx));
        bind_idx += 1;
    }
    if params.source.is_some() {
        query_str.push_str(&format!(" AND cl.source = ${}", bind_idx));
        bind_idx += 1;
    }
    if params.priority.is_some() {
        query_str.push_str(&format!(" AND cl.priority = ${}", bind_idx));
        bind_idx += 1;
    }
    if params.search.is_some() {
        query_str.push_str(&format!(
            " AND (cl.name ILIKE ${} OR cl.phone ILIKE ${})",
            bind_idx, bind_idx
        ));
        bind_idx += 1;
    }

    query_str.push_str(&format!(
        " ORDER BY cl.created_at DESC LIMIT ${} OFFSET ${}",
        bind_idx,
        bind_idx + 1
    ));

    let mut query = sqlx::query(&query_str).bind(user_uuid);
    if let Some(status) = &params.status {
        query = query.bind(status.clone());
    }
    if let Some(source) = &params.source {
        query = query.bind(source.clone());
    }
    if let Some(priority) = &params.priority {
        query = query.bind(priority.clone());
    }
    if let Some(search) = &params.search {
        query = query.bind(format!("%{}%", search));
    }
    query = query.bind(limit).bind(offset);

    let mut leads = Vec::new();
    match query.fetch_all(&state.db).await {
        Ok(rows) => {
            for row in rows {
                leads.push(crm_lead_from_row(&row));
            }
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!(ApiResponse {
                    success: false,
                    message: e.to_string(),
                    data: json!(null),
                })),
            )
                .into_response();
        }
    }

    // Total count (respecting the same filters, minus limit/offset)
    let mut count_query_str = String::from(
        "SELECT COUNT(*) FROM builder_crm_leads cl WHERE cl.user_id = $1",
    );
    let mut count_idx = 2;
    if params.status.is_some() {
        count_query_str.push_str(&format!(" AND cl.status = ${}", count_idx));
        count_idx += 1;
    }
    if params.source.is_some() {
        count_query_str.push_str(&format!(" AND cl.source = ${}", count_idx));
        count_idx += 1;
    }
    if params.priority.is_some() {
        count_query_str.push_str(&format!(" AND cl.priority = ${}", count_idx));
        count_idx += 1;
    }
    if params.search.is_some() {
        count_query_str.push_str(&format!(
            " AND (cl.name ILIKE ${} OR cl.phone ILIKE ${})",
            count_idx, count_idx
        ));
    }

    let mut count_query = sqlx::query_scalar(&count_query_str).bind(user_uuid);
    if let Some(status) = &params.status {
        count_query = count_query.bind(status.clone());
    }
    if let Some(source) = &params.source {
        count_query = count_query.bind(source.clone());
    }
    if let Some(priority) = &params.priority {
        count_query = count_query.bind(priority.clone());
    }
    if let Some(search) = &params.search {
        count_query = count_query.bind(format!("%{}%", search));
    }
    let total: i64 = count_query.fetch_one(&state.db).await.unwrap_or(0);

    let response = CrmLeadListResponse {
        leads,
        pagination: CrmLeadPagination {
            total,
            limit,
            offset,
        },
    };

    (
        StatusCode::OK,
        Json(json!(ApiResponse {
            success: true,
            message: "CRM leads fetched".to_string(),
            data: response,
        })),
    )
        .into_response()
}

// -----------------------------------------------------------------------------
// POST /api/builder/crm-leads
// -----------------------------------------------------------------------------
pub async fn create_crm_lead(
    State(state): State<AppState>,
    auth_user: AuthenticationUser,
    Json(payload): Json<CrmLeadPayload>,
) -> impl IntoResponse {
    if let Err(err) = verify_builder_role(&state, &auth_user.user_id).await {
        return err.into_response();
    }

    let user_uuid = Uuid::parse_str(&auth_user.user_id).unwrap_or_default();
    let status = payload.status.clone().unwrap_or_else(|| "new".to_string());
    let priority = payload.priority.clone().unwrap_or_else(|| "warm".to_string());

    if !VALID_SOURCES.contains(&payload.source.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "Invalid source", "data": null}))).into_response();
    }
    if !VALID_STATUSES.contains(&status.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "Invalid status", "data": null}))).into_response();
    }
    if !VALID_PRIORITIES.contains(&priority.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "Invalid priority", "data": null}))).into_response();
    }

    let inserted = sqlx::query(
        "INSERT INTO builder_crm_leads
            (user_id, project_id, source, source_detail, name, phone, email,
             budget_min, budget_max, requirement, location_preference,
             status, priority, notes, next_follow_up_date)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
         RETURNING id",
    )
    .bind(user_uuid)
    .bind(payload.project_id)
    .bind(&payload.source)
    .bind(&payload.source_detail)
    .bind(&payload.name)
    .bind(&payload.phone)
    .bind(&payload.email)
    .bind(payload.budget_min)
    .bind(payload.budget_max)
    .bind(&payload.requirement)
    .bind(&payload.location_preference)
    .bind(&status)
    .bind(&priority)
    .bind(&payload.notes)
    .bind(payload.next_follow_up_date)
    .fetch_one(&state.db)
    .await;

    let new_id: Uuid = match inserted {
        Ok(row) => row.get("id"),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!(ApiResponse {
                    success: false,
                    message: e.to_string(),
                    data: json!(null),
                })),
            )
                .into_response();
        }
    };

    let row = sqlx::query(
        "SELECT cl.*, bp.project_name AS project_name
         FROM builder_crm_leads cl
         LEFT JOIN builder_projects bp ON cl.project_id = bp.id
         WHERE cl.id = $1",
    )
    .bind(new_id)
    .fetch_one(&state.db)
    .await;

    match row {
        Ok(row) => (
            StatusCode::CREATED,
            Json(json!(ApiResponse {
                success: true,
                message: "CRM lead created".to_string(),
                data: crm_lead_from_row(&row),
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!(ApiResponse {
                success: false,
                message: e.to_string(),
                data: json!(null),
            })),
        )
            .into_response(),
    }
}

// -----------------------------------------------------------------------------
// PUT /api/builder/crm-leads/{id}
// -----------------------------------------------------------------------------
pub async fn update_crm_lead(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    auth_user: AuthenticationUser,
    Json(payload): Json<CrmLeadPayload>,
) -> impl IntoResponse {
    if let Err(err) = verify_builder_role(&state, &auth_user.user_id).await {
        return err.into_response();
    }

    let user_uuid = Uuid::parse_str(&auth_user.user_id).unwrap_or_default();
    let lead_uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!(ApiResponse {
                    success: false,
                    message: "Invalid lead ID".to_string(),
                    data: json!(null)
                })),
            )
                .into_response();
        }
    };

    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM builder_crm_leads WHERE id = $1 AND user_id = $2)",
    )
    .bind(lead_uuid)
    .bind(user_uuid)
    .fetch_one(&state.db)
    .await
    .unwrap_or(false);

    if !exists {
        return (
            StatusCode::NOT_FOUND,
            Json(json!(ApiResponse {
                success: false,
                message: "Lead not found".to_string(),
                data: json!(null)
            })),
        )
            .into_response();
    }

    let status = payload.status.clone().unwrap_or_else(|| "new".to_string());
    let priority = payload.priority.clone().unwrap_or_else(|| "warm".to_string());

    if !VALID_SOURCES.contains(&payload.source.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "Invalid source", "data": null}))).into_response();
    }
    if !VALID_STATUSES.contains(&status.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "Invalid status", "data": null}))).into_response();
    }
    if !VALID_PRIORITIES.contains(&priority.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "Invalid priority", "data": null}))).into_response();
    }

    let update = sqlx::query(
        "UPDATE builder_crm_leads SET
            project_id = $1, source = $2, source_detail = $3, name = $4, phone = $5,
            email = $6, budget_min = $7, budget_max = $8, requirement = $9,
            location_preference = $10, status = $11, priority = $12, notes = $13,
            next_follow_up_date = $14, updated_at = NOW()
         WHERE id = $15 AND user_id = $16",
    )
    .bind(payload.project_id)
    .bind(&payload.source)
    .bind(&payload.source_detail)
    .bind(&payload.name)
    .bind(&payload.phone)
    .bind(&payload.email)
    .bind(payload.budget_min)
    .bind(payload.budget_max)
    .bind(&payload.requirement)
    .bind(&payload.location_preference)
    .bind(&status)
    .bind(&priority)
    .bind(&payload.notes)
    .bind(payload.next_follow_up_date)
    .bind(lead_uuid)
    .bind(user_uuid)
    .execute(&state.db)
    .await;

    if let Err(e) = update {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!(ApiResponse {
                success: false,
                message: e.to_string(),
                data: json!(null),
            })),
        )
            .into_response();
    }

    let row = sqlx::query(
        "SELECT cl.*, bp.project_name AS project_name
         FROM builder_crm_leads cl
         LEFT JOIN builder_projects bp ON cl.project_id = bp.id
         WHERE cl.id = $1",
    )
    .bind(lead_uuid)
    .fetch_one(&state.db)
    .await;

    match row {
        Ok(row) => (
            StatusCode::OK,
            Json(json!(ApiResponse {
                success: true,
                message: "CRM lead updated".to_string(),
                data: crm_lead_from_row(&row),
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!(ApiResponse {
                success: false,
                message: e.to_string(),
                data: json!(null),
            })),
        )
            .into_response(),
    }
}

// -----------------------------------------------------------------------------
// PATCH /api/builder/crm-leads/{id}/status
// -----------------------------------------------------------------------------
pub async fn update_crm_lead_status(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    auth_user: AuthenticationUser,
    Json(payload): Json<CrmLeadStatusUpdate>,
) -> impl IntoResponse {
    if let Err(err) = verify_builder_role(&state, &auth_user.user_id).await {
        return err.into_response();
    }

    let user_uuid = Uuid::parse_str(&auth_user.user_id).unwrap_or_default();
    let lead_uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!(ApiResponse {
                    success: false,
                    message: "Invalid lead ID".to_string(),
                    data: json!(null)
                })),
            )
                .into_response();
        }
    };

    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM builder_crm_leads WHERE id = $1 AND user_id = $2)",
    )
    .bind(lead_uuid)
    .bind(user_uuid)
    .fetch_one(&state.db)
    .await
    .unwrap_or(false);

    if !exists {
        return (
            StatusCode::NOT_FOUND,
            Json(json!(ApiResponse {
                success: false,
                message: "Lead not found".to_string(),
                data: json!(null)
            })),
        )
            .into_response();
    }

    if !VALID_STATUSES.contains(&payload.status.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "Invalid status", "data": null}))).into_response();
    }

    let update = sqlx::query(
        "UPDATE builder_crm_leads SET status = $1, updated_at = NOW() WHERE id = $2 AND user_id = $3",
    )
    .bind(&payload.status)
    .bind(lead_uuid)
    .bind(user_uuid)
    .execute(&state.db)
    .await;

    match update {
        Ok(_) => (
            StatusCode::OK,
            Json(json!(ApiResponse {
                success: true,
                message: "Lead status updated".to_string(),
                data: json!({ "id": lead_uuid, "status": payload.status }),
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!(ApiResponse {
                success: false,
                message: e.to_string(),
                data: json!(null),
            })),
        )
            .into_response(),
    }
}

// -----------------------------------------------------------------------------
// DELETE /api/builder/crm-leads/{id}
// -----------------------------------------------------------------------------
pub async fn delete_crm_lead(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    auth_user: AuthenticationUser,
) -> impl IntoResponse {
    if let Err(err) = verify_builder_role(&state, &auth_user.user_id).await {
        return err.into_response();
    }

    let user_uuid = Uuid::parse_str(&auth_user.user_id).unwrap_or_default();
    let lead_uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!(ApiResponse {
                    success: false,
                    message: "Invalid lead ID".to_string(),
                    data: json!(null)
                })),
            )
                .into_response();
        }
    };

    let result = sqlx::query("DELETE FROM builder_crm_leads WHERE id = $1 AND user_id = $2")
        .bind(lead_uuid)
        .bind(user_uuid)
        .execute(&state.db)
        .await;

    match result {
        Ok(res) if res.rows_affected() > 0 => (
            StatusCode::OK,
            Json(json!(ApiResponse {
                success: true,
                message: "CRM lead deleted".to_string(),
                data: json!(null),
            })),
        )
            .into_response(),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(json!(ApiResponse {
                success: false,
                message: "Lead not found".to_string(),
                data: json!(null),
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!(ApiResponse {
                success: false,
                message: e.to_string(),
                data: json!(null),
            })),
        )
            .into_response(),
    }
}