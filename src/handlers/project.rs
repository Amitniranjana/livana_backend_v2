use axum::{
    extract::{Json as ExtractJson, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::models::project::{
    BuilderProject, BuilderProjectWithStats, CreateProjectLeadRequest, CreateProjectRequest,
    ProjectBuilderInfo, ProjectDetailResponse, ProjectReviewSummary, UpdateProjectRequest,
};
use crate::utils::auth_extractor::AuthenticationUser;

/// 4.1 POST /api/builder/projects
pub async fn create_project(
    auth: AuthenticationUser,
    State(app_state): State<AppState>,
    ExtractJson(payload): ExtractJson<CreateProjectRequest>,
) -> impl IntoResponse {
    let user_id = match Uuid::parse_str(&auth.user_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"success": false, "message": "Invalid user ID"})),
            )
                .into_response();
        }
    };

    // Check builder role (optional if auth middleware already checks, but good to be safe)
    let role: Option<String> = sqlx::query_scalar("SELECT user_role FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&app_state.db)
        .await
        .unwrap_or(None);

    if role.unwrap_or_default().to_lowercase() != "builder" {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"success": false, "message": "Only builders can create projects"})),
        )
            .into_response();
    }

    let result = sqlx::query_as::<_, BuilderProject>(
        r#"
        INSERT INTO builder_projects (
            user_id, project_name, project_type, status, description,
            city, locality, address, latitude, longitude, rera_id,
            total_units, total_towers, unit_configurations, price_range_min,
            price_range_max, area_range_min_sqft, area_range_max_sqft,
            possession_date, launch_date, amenities, nearby_places, images,
            brochure_url, video_url, master_plan_image_url, floor_plans
        )
        VALUES (
            $1, $2, $3, 'PENDING_REVIEW', $4,
            $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14,
            $15, $16, $17,
            $18, $19, $20, $21, $22,
            $23, $24, $25, $26
        )
        RETURNING *
        "#,
    )
    .bind(user_id)
    .bind(payload.project_name)
    .bind(payload.project_type)
    .bind(payload.description)
    .bind(payload.city)
    .bind(payload.locality)
    .bind(payload.address)
    .bind(payload.latitude)
    .bind(payload.longitude)
    .bind(payload.rera_id)
    .bind(payload.total_units)
    .bind(payload.total_towers)
    .bind(payload.unit_configurations)
    .bind(payload.price_range_min)
    .bind(payload.price_range_max)
    .bind(payload.area_range_min_sqft)
    .bind(payload.area_range_max_sqft)
    .bind(payload.possession_date)
    .bind(payload.launch_date)
    .bind(payload.amenities)
    .bind(payload.nearby_places)
    .bind(payload.images)
    .bind(payload.brochure_url)
    .bind(payload.video_url)
    .bind(payload.master_plan_image_url)
    .bind(payload.floor_plans)
    .fetch_one(&app_state.db)
    .await;

    match result {
        Ok(project) => (
            StatusCode::CREATED,
            Json(json!({
                "success": true,
                "message": "Project created successfully",
                "data": project
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "message": format!("Database error: {}", e)
            })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct ProjectQueryParams {
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// 4.2 GET /api/builder/projects
pub async fn get_builder_projects(
    auth: AuthenticationUser,
    State(app_state): State<AppState>,
    Query(params): Query<ProjectQueryParams>,
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

    let mut query = r#"
        SELECT
            bp.*,
            (SELECT COUNT(*) FROM properties WHERE project_id = bp.id) AS units_sold,
            (SELECT COUNT(*) FROM site_visits WHERE project_id = bp.id) AS visits_count,
            (SELECT COUNT(*) FROM project_leads WHERE project_id = bp.id) AS leads_count
        FROM builder_projects bp
        WHERE bp.user_id = $1
    "#.to_string();
    
    if let Some(ref s) = params.status {
        if s != "all" {
            query.push_str(&format!(" AND bp.status = '{}'", s));
        }
    }
    
    query.push_str(" ORDER BY bp.created_at DESC LIMIT $2 OFFSET $3");

    #[derive(sqlx::FromRow)]
    struct RowWithStats {
        #[sqlx(flatten)]
        project: BuilderProject,
        units_sold: Option<i64>,
        visits_count: Option<i64>,
        leads_count: Option<i64>,
    }

    let projects = sqlx::query_as::<_, RowWithStats>(&query)
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&app_state.db)
        .await;

    match projects {
        Ok(projs) => {
            let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM builder_projects WHERE user_id = $1")
                .bind(user_id)
                .fetch_one(&app_state.db)
                .await
                .unwrap_or(0);
                
            let stats_projs: Vec<BuilderProjectWithStats> = projs.into_iter().map(|p| BuilderProjectWithStats {
                project: p.project,
                units_sold: p.units_sold.unwrap_or(0),
                visits_count: p.visits_count.unwrap_or(0),
                leads_count: p.leads_count.unwrap_or(0),
            }).collect();
                
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "data": {
                        "projects": stats_projs,
                        "pagination": { "total": total, "limit": limit, "offset": offset }
                    }
                })),
            ).into_response()
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": format!("Error: {}", e)})),
        ).into_response()
    }
}

/// 4.3 GET /api/projects/{id}
pub async fn get_project_by_id(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let project = sqlx::query_as::<_, BuilderProject>(
        "SELECT * FROM builder_projects WHERE id = $1 AND status != 'deleted'"
    )
    .bind(id)
    .fetch_optional(&app_state.db)
    .await;
    
    match project {
        Ok(Some(p)) => {
            // Also update views_count
            let _ = sqlx::query("UPDATE builder_projects SET views_count = views_count + 1 WHERE id = $1")
                .bind(id)
                .execute(&app_state.db)
                .await;
                
            // Fetch Builder Info
            #[derive(sqlx::FromRow)]
            struct BInfo {
                id: Uuid,
                name: Option<String>,
                logo: Option<String>,
                is_verified: Option<bool>,
            }
            let b_info = sqlx::query_as::<_, BInfo>(
                "SELECT u.id, (u.first_name || ' ' || u.last_name) AS name, bp.logo_url AS logo, bp.is_verified
                 FROM users u
                 LEFT JOIN builder_profiles bp ON u.id = bp.user_id
                 WHERE u.id = $1"
            ).bind(p.user_id).fetch_optional(&app_state.db).await.unwrap_or(None);
            
            let builder_info = if let Some(info) = b_info {
                ProjectBuilderInfo {
                    id: info.id,
                    name: info.name.unwrap_or_else(|| "Unknown Builder".to_string()),
                    logo: info.logo,
                    is_verified: info.is_verified.unwrap_or(false),
                }
            } else {
                ProjectBuilderInfo {
                    id: p.user_id,
                    name: "Unknown Builder".to_string(),
                    logo: None,
                    is_verified: false,
                }
            };
            
            // Fetch review summary
            let review_summary: (Option<f64>, Option<i64>) = sqlx::query_as(
                "SELECT AVG(rating), COUNT(*) FROM property_reviews WHERE project_id = $1"
            ).bind(id).fetch_one(&app_state.db).await.unwrap_or((None, None));
            
            let review_summary = ProjectReviewSummary {
                average_rating: review_summary.0.unwrap_or(0.0),
                total_reviews: review_summary.1.unwrap_or(0),
            };
            
            // Fetch related units (properties)
            let units = sqlx::query_scalar::<_, serde_json::Value>(
                "SELECT row_to_json(p) FROM properties p WHERE project_id = $1"
            ).bind(id).fetch_all(&app_state.db).await.unwrap_or(vec![]);
            
            let response = ProjectDetailResponse {
                project: p,
                builder_info,
                review_summary,
                related_units: units,
            };

            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "data": response
                }))
            ).into_response()
        },
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "Project not found"}))
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": format!("Error: {}", e)}))
        ).into_response()
    }
}

/// 4.4 PUT /api/builder/projects/{id}
pub async fn update_project(
    auth: AuthenticationUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
    ExtractJson(payload): ExtractJson<UpdateProjectRequest>,
) -> impl IntoResponse {
    let user_id = match Uuid::parse_str(&auth.user_id) {
        Ok(uid) => uid,
        Err(_) => return (StatusCode::UNAUTHORIZED, Json(json!({"success": false}))).into_response(),
    };
    
    let result = sqlx::query_as::<_, BuilderProject>(
        r#"
        UPDATE builder_projects
        SET project_name = COALESCE($3, project_name),
            project_type = COALESCE($4, project_type),
            status = COALESCE($5, status),
            description = COALESCE($6, description),
            city = COALESCE($7, city),
            locality = COALESCE($8, locality),
            address = COALESCE($9, address),
            latitude = COALESCE($10, latitude),
            longitude = COALESCE($11, longitude),
            rera_id = COALESCE($12, rera_id),
            total_units = COALESCE($13, total_units),
            total_towers = COALESCE($14, total_towers),
            unit_configurations = COALESCE($15, unit_configurations),
            price_range_min = COALESCE($16, price_range_min),
            price_range_max = COALESCE($17, price_range_max),
            area_range_min_sqft = COALESCE($18, area_range_min_sqft),
            area_range_max_sqft = COALESCE($19, area_range_max_sqft),
            possession_date = COALESCE($20, possession_date),
            launch_date = COALESCE($21, launch_date),
            amenities = COALESCE($22, amenities),
            nearby_places = COALESCE($23, nearby_places),
            images = COALESCE($24, images),
            brochure_url = COALESCE($25, brochure_url),
            video_url = COALESCE($26, video_url),
            master_plan_image_url = COALESCE($27, master_plan_image_url),
            floor_plans = COALESCE($28, floor_plans),
            updated_at = NOW()
        WHERE id = $1 AND user_id = $2
        RETURNING *
        "#
    )
    .bind(id)
    .bind(user_id)
    .bind(payload.project_name)
    .bind(payload.project_type)
    .bind(payload.status)
    .bind(payload.description)
    .bind(payload.city)
    .bind(payload.locality)
    .bind(payload.address)
    .bind(payload.latitude)
    .bind(payload.longitude)
    .bind(payload.rera_id)
    .bind(payload.total_units)
    .bind(payload.total_towers)
    .bind(payload.unit_configurations)
    .bind(payload.price_range_min)
    .bind(payload.price_range_max)
    .bind(payload.area_range_min_sqft)
    .bind(payload.area_range_max_sqft)
    .bind(payload.possession_date)
    .bind(payload.launch_date)
    .bind(payload.amenities)
    .bind(payload.nearby_places)
    .bind(payload.images)
    .bind(payload.brochure_url)
    .bind(payload.video_url)
    .bind(payload.master_plan_image_url)
    .bind(payload.floor_plans)
    .fetch_one(&app_state.db)
    .await;
    
    match result {
        Ok(project) => (
            StatusCode::OK,
            Json(json!({"success": true, "data": project}))
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": format!("Error: {}", e)}))
        ).into_response()
    }
}

/// 4.5 DELETE /api/builder/projects/{id}
pub async fn delete_project(
    auth: AuthenticationUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let user_id = match Uuid::parse_str(&auth.user_id) {
        Ok(uid) => uid,
        Err(_) => return (StatusCode::UNAUTHORIZED, Json(json!({"success": false}))).into_response(),
    };
    
    let result = sqlx::query("UPDATE builder_projects SET status = 'deleted' WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(&app_state.db)
        .await;
        
    match result {
        Ok(_) => (StatusCode::OK, Json(json!({"success": true, "message": "Project deleted successfully"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": format!("Error: {}", e)}))).into_response()
    }
}

#[derive(Deserialize)]
pub struct SearchProjectsParams {
    pub city: Option<String>,
    pub project_type: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// 4.6 GET /api/projects
pub async fn search_projects(
    State(app_state): State<AppState>,
    Query(params): Query<SearchProjectsParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);
    
    let mut query = "SELECT * FROM builder_projects WHERE status != 'deleted'".to_string();
    
    if let Some(ref c) = params.city {
        query.push_str(&format!(" AND city ILIKE '%{}%'", c));
    }
    if let Some(ref pt) = params.project_type {
        query.push_str(&format!(" AND project_type = '{}'", pt));
    }
    if let Some(ref s) = params.status {
        query.push_str(&format!(" AND status = '{}'", s));
    }
    
    query.push_str(" ORDER BY created_at DESC LIMIT $1 OFFSET $2");
    
    let projects = sqlx::query_as::<_, BuilderProject>(&query)
        .bind(limit)
        .bind(offset)
        .fetch_all(&app_state.db)
        .await;
        
    match projects {
        Ok(projs) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "data": {
                    "projects": projs,
                    "pagination": { "limit": limit, "offset": offset }
                }
            }))
        ).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": format!("Error: {}", e)}))).into_response()
    }
}

/// 4.8 POST /api/projects/{id}/enquire
pub async fn enquire_project(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
    ExtractJson(payload): ExtractJson<CreateProjectLeadRequest>,
) -> impl IntoResponse {
    let result = sqlx::query(
        "INSERT INTO project_leads (project_id, name, phone, message, preferred_visit_date) VALUES ($1, $2, $3, $4, $5)"
    )
    .bind(id)
    .bind(payload.name)
    .bind(payload.phone)
    .bind(payload.message)
    .bind(payload.preferred_visit_date)
    .execute(&app_state.db)
    .await;
    
    match result {
        Ok(_) => (StatusCode::OK, Json(json!({"success": true, "message": "Enquiry submitted successfully"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": format!("Error: {}", e)}))).into_response()
    }
}

#[derive(Deserialize)]
pub struct AttachUnitRequest {
    pub property_id: Uuid,
}

/// 4.7 POST /api/builder/projects/{id}/units
pub async fn attach_unit_to_project(
    auth: AuthenticationUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
    ExtractJson(payload): ExtractJson<AttachUnitRequest>,
) -> impl IntoResponse {
    let user_id = match Uuid::parse_str(&auth.user_id) {
        Ok(uid) => uid,
        Err(_) => return (StatusCode::UNAUTHORIZED, Json(json!({"success": false}))).into_response(),
    };
    
    // Verify owner of project
    let owner_check: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM builder_projects WHERE id = $1")
        .bind(id)
        .fetch_optional(&app_state.db)
        .await
        .unwrap_or(None);
        
    if owner_check != Some(user_id) {
        return (StatusCode::FORBIDDEN, Json(json!({"success": false, "message": "Not owner of project"}))).into_response();
    }
    
    // Verify owner of property
    let prop_owner: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM properties WHERE id = $1")
        .bind(payload.property_id)
        .fetch_optional(&app_state.db)
        .await
        .unwrap_or(None);
        
    if prop_owner != Some(user_id) {
        return (StatusCode::FORBIDDEN, Json(json!({"success": false, "message": "Not owner of property"}))).into_response();
    }
    
    let result = sqlx::query("UPDATE properties SET project_id = $1 WHERE id = $2")
        .bind(id)
        .bind(payload.property_id)
        .execute(&app_state.db)
        .await;
        
    match result {
        Ok(_) => (StatusCode::OK, Json(json!({"success": true, "message": "Unit attached successfully"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": format!("Error: {}", e)}))).into_response()
    }
}
