use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::{
        builder_crm::{
            CrmLeadListResponse, CrmLeadPagination, CrmLeadPayload, CrmLeadQuery,
            CrmLeadResponse, CrmLeadStatusUpdate,
        },
        response::ApiResponse,
    },
    utils::auth_extractor::AuthenticationUser,
};

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

    let limit = params.limit.unwrap_or(50).clamp(1, 1000);
    let offset = params.offset.unwrap_or(0).max(0);

    let mut query = sqlx::QueryBuilder::new(
        "SELECT l.*, p.name as project_name 
         FROM builder_crm_leads l 
         LEFT JOIN builder_projects p ON l.project_id = p.id 
         WHERE l.user_id = "
    );
    query.push_bind(user_uuid);

    let mut count_query = sqlx::QueryBuilder::new(
        "SELECT COUNT(*) FROM builder_crm_leads l WHERE l.user_id = "
    );
    count_query.push_bind(user_uuid);

    if let Some(status) = &params.status {
        query.push(" AND l.status = ");
        query.push_bind(status);
        count_query.push(" AND l.status = ");
        count_query.push_bind(status);
    }
    
    if let Some(source) = &params.source {
        query.push(" AND l.source = ");
        query.push_bind(source);
        count_query.push(" AND l.source = ");
        count_query.push_bind(source);
    }
    
    if let Some(priority) = &params.priority {
        query.push(" AND l.priority = ");
        query.push_bind(priority);
        count_query.push(" AND l.priority = ");
        count_query.push_bind(priority);
    }

    if let Some(search) = &params.search {
        if !search.trim().is_empty() {
            let search_term = format!("%{}%", search.trim());
            let search_clause: &str = " AND (l.name ILIKE ";
            query.push(search_clause);
            query.push_bind(search_term.clone());
            query.push(" OR l.phone ILIKE ");
            query.push_bind(search_term.clone());
            query.push(" OR l.email ILIKE ");
            query.push_bind(search_term.clone());
            query.push(")");

            count_query.push(search_clause);
            count_query.push_bind(search_term.clone());
            count_query.push(" OR l.phone ILIKE ");
            count_query.push_bind(search_term.clone());
            count_query.push(" OR l.email ILIKE ");
            count_query.push_bind(search_term.clone());
            count_query.push(")");
        }
    }

    query.push(" ORDER BY l.created_at DESC LIMIT ");
    query.push_bind(limit);
    query.push(" OFFSET ");
    query.push_bind(offset);

    let total: i64 = match count_query.build_query_scalar().fetch_one(&state.db).await {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse { success: false, data: (), message: "DB error".into() })).into_response(),
    };

    let rows = match query.build().fetch_all(&state.db).await {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": format!("DB error: {}", e), "data": null}))).into_response(),
    };

    let mut leads = Vec::new();
    for row in rows {
        leads.push(CrmLeadResponse {
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
        });
    }

    (
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            data: CrmLeadListResponse {
                leads,
                pagination: CrmLeadPagination { total, limit, offset },
            },
            message: "".into(),
        }),
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

    let status = payload.status.unwrap_or_else(|| "new".to_string());
    let priority = payload.priority.unwrap_or_else(|| "warm".to_string());

    if !VALID_SOURCES.contains(&payload.source.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "Invalid source", "data": null}))).into_response();
    }
    if !VALID_STATUSES.contains(&status.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "Invalid status", "data": null}))).into_response();
    }
    if !VALID_PRIORITIES.contains(&priority.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "Invalid priority", "data": null}))).into_response();
    }

    let row = match sqlx::query(
        r#"
        INSERT INTO builder_crm_leads (
            user_id, project_id, source, source_detail, name, phone, email, 
            budget_min, budget_max, requirement, location_preference, status, 
            priority, notes, next_follow_up_date
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15
        ) RETURNING id, user_id, project_id, source, source_detail, name, phone, email, 
                  budget_min, budget_max, requirement, location_preference, status, 
                  priority, notes, next_follow_up_date, created_at, updated_at
        "#)
        .bind(user_uuid)
        .bind(payload.project_id)
        .bind(payload.source)
        .bind(payload.source_detail)
        .bind(payload.name)
        .bind(payload.phone)
        .bind(payload.email)
        .bind(payload.budget_min)
        .bind(payload.budget_max)
        .bind(payload.requirement)
        .bind(payload.location_preference)
        .bind(status)
        .bind(priority)
        .bind(payload.notes)
        .bind(payload.next_follow_up_date as Option<chrono::NaiveDate>)
    .fetch_one(&state.db)
    .await
    {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": format!("DB error: {}", e), "data": null}))).into_response(),
    };

    let mut project_name = None;
    if let Some(pid) = row.try_get::<Option<Uuid>, _>("project_id").unwrap_or(None) {
        project_name = sqlx::query_scalar("SELECT name FROM builder_projects WHERE id = $1")
            .bind(pid)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);
    }

    let lead = CrmLeadResponse {
        id: row.get("id"),
        user_id: row.get("user_id"),
        project_id: row.try_get("project_id").unwrap_or(None),
        project_name,
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
        created_at: row.try_get("created_at").unwrap_or_default(),
        updated_at: row.try_get("updated_at").unwrap_or_default(),
    };

    (StatusCode::OK, Json(ApiResponse { success: true, data: lead, message: "CRM Lead created".into() })).into_response()
}

// -----------------------------------------------------------------------------
// PUT /api/builder/crm-leads/:id
// -----------------------------------------------------------------------------
pub async fn update_crm_lead(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth_user: AuthenticationUser,
    Json(payload): Json<CrmLeadPayload>,
) -> impl IntoResponse {
    if let Err(err) = verify_builder_role(&state, &auth_user.user_id).await {
        return err.into_response();
    }
    let user_uuid = Uuid::parse_str(&auth_user.user_id).unwrap_or_default();

    // Verify ownership
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM builder_crm_leads WHERE id = $1 AND user_id = $2)")
        .bind(id)
        .bind(user_uuid)
        .fetch_one(&state.db)
        .await
        .unwrap_or(false);

    if !exists {
        return (StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "Lead not found", "data": null}))).into_response();
    }

    let status = payload.status.unwrap_or_else(|| "new".to_string());
    let priority = payload.priority.unwrap_or_else(|| "warm".to_string());

    if !VALID_SOURCES.contains(&payload.source.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "Invalid source", "data": null}))).into_response();
    }
    if !VALID_STATUSES.contains(&status.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "Invalid status", "data": null}))).into_response();
    }
    if !VALID_PRIORITIES.contains(&priority.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "Invalid priority", "data": null}))).into_response();
    }

    let row = match sqlx::query(
        r#"
        UPDATE builder_crm_leads SET
            project_id = $1, source = $2, source_detail = $3, name = $4, phone = $5, email = $6,
            budget_min = $7, budget_max = $8, requirement = $9, location_preference = $10,
            status = $11, priority = $12, notes = $13, next_follow_up_date = $14, updated_at = NOW()
        WHERE id = $15 AND user_id = $16
        RETURNING id, user_id, project_id, source, source_detail, name, phone, email, 
                  budget_min, budget_max, requirement, location_preference, status, 
                  priority, notes, next_follow_up_date, created_at, updated_at
        "#)
        .bind(payload.project_id)
        .bind(payload.source)
        .bind(payload.source_detail)
        .bind(payload.name)
        .bind(payload.phone)
        .bind(payload.email)
        .bind(payload.budget_min)
        .bind(payload.budget_max)
        .bind(payload.requirement)
        .bind(payload.location_preference)
        .bind(status)
        .bind(priority)
        .bind(payload.notes)
        .bind(payload.next_follow_up_date as Option<chrono::NaiveDate>)
        .bind(id)
        .bind(user_uuid)
    .fetch_one(&state.db)
    .await
    {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": format!("DB error: {}", e), "data": null}))).into_response(),
    };

    let mut project_name = None;
    if let Some(pid) = row.try_get::<Option<Uuid>, _>("project_id").unwrap_or(None) {
        project_name = sqlx::query_scalar("SELECT name FROM builder_projects WHERE id = $1")
            .bind(pid)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);
    }

    let lead = CrmLeadResponse {
        id: row.get("id"),
        user_id: row.get("user_id"),
        project_id: row.try_get("project_id").unwrap_or(None),
        project_name,
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
        created_at: row.try_get("created_at").unwrap_or_default(),
        updated_at: row.try_get("updated_at").unwrap_or_default(),
    };

    (StatusCode::OK, Json(ApiResponse { success: true, data: lead, message: "CRM Lead updated".into() })).into_response()
}

// -----------------------------------------------------------------------------
// PATCH /api/builder/crm-leads/:id/status
// -----------------------------------------------------------------------------
pub async fn update_crm_lead_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth_user: AuthenticationUser,
    Json(payload): Json<CrmLeadStatusUpdate>,
) -> impl IntoResponse {
    if let Err(err) = verify_builder_role(&state, &auth_user.user_id).await {
        return err.into_response();
    }
    let user_uuid = Uuid::parse_str(&auth_user.user_id).unwrap_or_default();

    // Verify ownership
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM builder_crm_leads WHERE id = $1 AND user_id = $2)")
        .bind(id)
        .bind(user_uuid)
        .fetch_one(&state.db)
        .await
        .unwrap_or(false);

    if !exists {
        return (StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "Lead not found", "data": null}))).into_response();
    }

    if !VALID_STATUSES.contains(&payload.status.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "Invalid status", "data": null}))).into_response();
    }

    match sqlx::query("UPDATE builder_crm_leads SET status = $1, updated_at = NOW() WHERE id = $2")
        .bind(&payload.status)
        .bind(id)
        .execute(&state.db)
        .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: json!({ "id": id, "status": payload.status }),
                message: "Status updated".into(),
            }),
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": format!("DB error: {}", e), "data": null}))).into_response(),
    }
}

// -----------------------------------------------------------------------------
// DELETE /api/builder/crm-leads/:id
// -----------------------------------------------------------------------------
pub async fn delete_crm_lead(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth_user: AuthenticationUser,
) -> impl IntoResponse {
    if let Err(err) = verify_builder_role(&state, &auth_user.user_id).await {
        return err.into_response();
    }
    let user_uuid = Uuid::parse_str(&auth_user.user_id).unwrap_or_default();

    // Verify ownership
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM builder_crm_leads WHERE id = $1 AND user_id = $2)")
        .bind(id)
        .bind(user_uuid)
        .fetch_one(&state.db)
        .await
        .unwrap_or(false);

    if !exists {
        return (StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "Lead not found", "data": null}))).into_response();
    }

    match sqlx::query("DELETE FROM builder_crm_leads WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse { success: true, data: json!(null), message: "Lead deleted".into() }),
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": format!("DB error: {}", e), "data": null}))).into_response(),
    }
}
