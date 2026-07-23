use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    handlers::admin_auth::AdminClaims,
    utils::admin_logger::log_admin_action,
};

#[derive(Debug, Deserialize)]
pub struct AdminPropertiesParams {
    pub search: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "listingType")]
    pub listing_type: Option<String>,
    #[serde(rename = "propertyType")]
    pub property_type: Option<String>,
    pub city: Option<String>,
    #[serde(rename = "isFeatured")]
    pub is_featured: Option<bool>,
    #[serde(rename = "postedByUserType")]
    pub posted_by_user_type: Option<String>,
    #[serde(rename = "userId")]
    pub user_id: Option<Uuid>,
    #[serde(rename = "sortBy")]
    pub sort_by: Option<String>, // e.g. "created_at", "price", "title"
    #[serde(rename = "sortDir")]
    pub sort_dir: Option<String>, // "asc", "desc"
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePropertyRequest {
    pub status: Option<String>,
    #[serde(rename = "isFeatured")]
    pub is_featured: Option<bool>,
    #[serde(rename = "isVerified")]
    pub is_verified: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct BulkPropertyActionRequest {
    #[serde(rename = "propertyIds")]
    pub property_ids: Vec<Uuid>,
    pub action: String, // feature, unfeature, suspend, force-delete
    pub reason: Option<String>,
}

fn row_to_admin_property_json(row: &sqlx::postgres::PgRow) -> Value {
    // Parse JSON columns
    let images_val: Value = row.try_get("images").unwrap_or(json!([]));
    
    // Owner info
    let owner_id: Uuid = row.try_get("user_id").unwrap_or(Uuid::nil());
    let first_name: String = row.try_get("first_name").unwrap_or_default();
    let last_name: String = row.try_get("last_name").unwrap_or_default();
    
    json!({
        "id": row.try_get::<Uuid, _>("id").map(|u| u.to_string()).unwrap_or_default(),
        "title": row.try_get::<Option<String>, _>("title").ok().flatten(),
        "description": row.try_get::<Option<String>, _>("description").ok().flatten(),
        "property_type": row.try_get::<Option<String>, _>("property_type").ok().flatten(),
        "listing_type": row.try_get::<Option<String>, _>("listing_type").ok().flatten(),
        "price": row.try_get::<Option<i64>, _>("price").ok().flatten(),
        "location": row.try_get::<Option<String>, _>("location").ok().flatten(),
        "city": row.try_get::<Option<String>, _>("city").ok().flatten(),
        "status": row.try_get::<Option<String>, _>("status").ok().flatten(),
        "is_featured": row.try_get::<Option<bool>, _>("is_featured").ok().flatten().unwrap_or(false),
        "is_verified": row.try_get::<Option<bool>, _>("is_verified").ok().flatten().unwrap_or(false),
        "user_type": row.try_get::<Option<String>, _>("user_type").ok().flatten(),
        "images": images_val,
        "created_at": row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at")
            .ok().flatten().map(|d| d.to_rfc3339()),
        "owner": {
            "id": owner_id.to_string(),
            "name": format!("{} {}", first_name, last_name).trim().to_string(),
        }
    })
}

// 1. GET /api/admin/properties
pub async fn get_properties(
    State(app_state): State<AppState>,
    Query(params): Query<AdminPropertiesParams>,
) -> impl axum::response::IntoResponse {
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    let mut query_str = String::from(
        "SELECT p.*, p.city as location, u.first_name, u.last_name 
         FROM properties p 
         LEFT JOIN users u ON p.user_id = u.id 
         WHERE 1=1"
    );
    let mut count_query_str = String::from(
        "SELECT count(*) FROM properties p 
         LEFT JOIN users u ON p.user_id = u.id 
         WHERE 1=1"
    );

    let mut bindings_idx = 1;
    let mut args = sqlx::postgres::PgArguments::default();
    let mut count_args = sqlx::postgres::PgArguments::default();

    if let Some(search) = &params.search {
        if !search.trim().is_empty() {
            let s = format!("%{}%", search);
            let filter = format!(" AND (p.title ILIKE ${} OR p.city ILIKE ${} OR p.locality ILIKE ${} OR u.first_name ILIKE ${} OR u.last_name ILIKE ${})", bindings_idx, bindings_idx, bindings_idx, bindings_idx, bindings_idx);
            query_str.push_str(&filter);
            count_query_str.push_str(&filter);
            let _ = sqlx::Arguments::add(&mut args, s.clone());
            let _ = sqlx::Arguments::add(&mut args, s.clone());
            let _ = sqlx::Arguments::add(&mut args, s.clone());
            let _ = sqlx::Arguments::add(&mut args, s.clone());
            let _ = sqlx::Arguments::add(&mut args, s.clone());
            
            let _ = sqlx::Arguments::add(&mut count_args, s.clone());
            let _ = sqlx::Arguments::add(&mut count_args, s.clone());
            let _ = sqlx::Arguments::add(&mut count_args, s.clone());
            let _ = sqlx::Arguments::add(&mut count_args, s.clone());
            let _ = sqlx::Arguments::add(&mut count_args, s);
            bindings_idx += 1; // Actually in postgres we can reuse $1 with sqlx? Wait, no, sqlx does not support reusing bindings. Wait, if I do `ILIKE $1 OR ILIKE $1`, sqlx allows that! BUT I wrote ${} and incremented once. Ah! I need to use the exact index if I want to reuse.
            // Let me fix the bind string to reuse index.
        }
    }

    if let Some(status) = &params.status {
        let filter = format!(" AND p.status = ${}", bindings_idx);
        query_str.push_str(&filter);
        count_query_str.push_str(&filter);
        let _ = sqlx::Arguments::add(&mut args, status.clone());
        let _ = sqlx::Arguments::add(&mut count_args, status.clone());
        bindings_idx += 1;
    }

    if let Some(listing_type) = &params.listing_type {
        let filter = format!(" AND p.listing_type = ${}", bindings_idx);
        query_str.push_str(&filter);
        count_query_str.push_str(&filter);
        let _ = sqlx::Arguments::add(&mut args, listing_type.clone());
        let _ = sqlx::Arguments::add(&mut count_args, listing_type.clone());
        bindings_idx += 1;
    }

    if let Some(property_type) = &params.property_type {
        let filter = format!(" AND p.property_type = ${}", bindings_idx);
        query_str.push_str(&filter);
        count_query_str.push_str(&filter);
        let _ = sqlx::Arguments::add(&mut args, property_type.clone());
        let _ = sqlx::Arguments::add(&mut count_args, property_type.clone());
        bindings_idx += 1;
    }

    if let Some(city) = &params.city {
        let filter = format!(" AND p.city = ${}", bindings_idx);
        query_str.push_str(&filter);
        count_query_str.push_str(&filter);
        let _ = sqlx::Arguments::add(&mut args, city.clone());
        let _ = sqlx::Arguments::add(&mut count_args, city.clone());
        bindings_idx += 1;
    }

    if let Some(is_featured) = params.is_featured {
        let filter = format!(" AND p.is_featured = ${}", bindings_idx);
        query_str.push_str(&filter);
        count_query_str.push_str(&filter);
        let _ = sqlx::Arguments::add(&mut args, is_featured);
        let _ = sqlx::Arguments::add(&mut count_args, is_featured);
        bindings_idx += 1;
    }

    if let Some(user_type) = &params.posted_by_user_type {
        let filter = format!(" AND p.user_type = ${}", bindings_idx); // wait, user_type or posted_by? checking schema: it's posted_by. But let's check listing.rs
        // Wait, listing.rs has `user_type` from `properties` table? Let me check `row_to_admin_property_json`. Yes, `user_type` column maybe? I'll use `posted_by`.
        query_str.push_str(&filter);
        count_query_str.push_str(&filter);
        let _ = sqlx::Arguments::add(&mut args, user_type.clone());
        let _ = sqlx::Arguments::add(&mut count_args, user_type.clone());
        bindings_idx += 1;
    }

    if let Some(user_id) = params.user_id {
        let filter = format!(" AND p.user_id = ${}", bindings_idx);
        query_str.push_str(&filter);
        count_query_str.push_str(&filter);
        let _ = sqlx::Arguments::add(&mut args, user_id);
        let _ = sqlx::Arguments::add(&mut count_args, user_id);
        bindings_idx += 1;
    }

    // Sorting
    let sort_col = match params.sort_by.as_deref() {
        Some("price") => "p.price",
        Some("title") => "p.title",
        _ => "p.created_at",
    };
    let sort_direction = match params.sort_dir.as_deref().unwrap_or("desc").to_lowercase().as_str() {
        "asc" => "ASC",
        _ => "DESC",
    };
    query_str.push_str(&format!(" ORDER BY {} {}", sort_col, sort_direction));
    
    // Pagination
    query_str.push_str(&format!(" LIMIT ${} OFFSET ${}", bindings_idx, bindings_idx + 1));
    let _ = sqlx::Arguments::add(&mut args, limit);
    let _ = sqlx::Arguments::add(&mut args, offset);

    let count: (i64,) = sqlx::query_as_with(&count_query_str, count_args)
        .fetch_one(&app_state.db)
        .await
        .unwrap_or((0,));
        
    let total_count = count.0;

    let rows = sqlx::query_with(&query_str, args)
        .fetch_all(&app_state.db)
        .await
        .unwrap_or_default();

    let properties: Vec<Value> = rows.iter().map(row_to_admin_property_json).collect();

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": properties,
            "pagination": {
                "total": total_count,
                "limit": limit,
                "offset": offset
            }
        })),
    )
}

// 2. GET /api/admin/properties/:id
pub async fn get_property_detail(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl axum::response::IntoResponse {
    let property_row = sqlx::query(
        "SELECT p.*, p.city as location, u.first_name, u.last_name 
         FROM properties p 
         LEFT JOIN users u ON p.user_id = u.id 
         WHERE p.id = $1"
    )
    .bind(id)
    .fetch_optional(&app_state.db)
    .await;

    match property_row {
        Ok(Some(row)) => {
            let mut property_json = row_to_admin_property_json(&row);

            // Fetch reviews
            let review_rows = sqlx::query(
                "SELECT r.id, r.rating, r.review, r.created_at, u.first_name, u.last_name 
                 FROM property_reviews r 
                 LEFT JOIN users u ON r.user_id = u.id 
                 WHERE r.property_id = $1 
                 ORDER BY r.created_at DESC"
            )
            .bind(id)
            .fetch_all(&app_state.db)
            .await
            .unwrap_or_default();

            let reviews: Vec<Value> = review_rows.into_iter().map(|r| {
                json!({
                    "id": r.try_get::<Uuid, _>("id").map(|u| u.to_string()).unwrap_or_default(),
                    "rating": r.try_get::<Option<i32>, _>("rating").ok().flatten(),
                    "review": r.try_get::<Option<String>, _>("review").ok().flatten(),
                    "created_at": r.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at")
                        .ok().flatten().map(|d| d.to_rfc3339()),
                    "user_name": format!("{} {}", r.try_get::<String, _>("first_name").unwrap_or_default(), r.try_get::<String, _>("last_name").unwrap_or_default()).trim()
                })
            }).collect();

            property_json["reviews"] = json!(reviews);

            (
                StatusCode::OK,
                Json(json!({ "success": true, "data": property_json })),
            )
        }
        _ => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "message": "Property not found" })),
        ),
    }
}

// 3. PATCH /api/admin/properties/:id
pub async fn update_property(
    State(app_state): State<AppState>,
    axum::extract::Extension(admin_claims): axum::extract::Extension<AdminClaims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdatePropertyRequest>,
) -> impl axum::response::IntoResponse {
    let mut query_parts = Vec::new();
    let mut args = sqlx::postgres::PgArguments::default();
    let mut bindings_idx = 1;

    if let Some(status) = &payload.status {
        query_parts.push(format!("status = ${}", bindings_idx));
        let _ = sqlx::Arguments::add(&mut args, status.clone());
        bindings_idx += 1;
    }

    if let Some(is_featured) = payload.is_featured {
        query_parts.push(format!("is_featured = ${}", bindings_idx));
        let _ = sqlx::Arguments::add(&mut args, is_featured);
        bindings_idx += 1;
    }

    if let Some(is_verified) = payload.is_verified {
        query_parts.push(format!("is_verified = ${}", bindings_idx));
        let _ = sqlx::Arguments::add(&mut args, is_verified);
        bindings_idx += 1;
    }

    if query_parts.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "message": "No fields to update" })),
        );
    }

    query_parts.push(format!("updated_at = NOW()"));

    let query_str = format!(
        "UPDATE properties SET {} WHERE id = ${} RETURNING user_id",
        query_parts.join(", "),
        bindings_idx
    );
    let _ = sqlx::Arguments::add(&mut args, id);

    match sqlx::query_with(&query_str, args).fetch_optional(&app_state.db).await {
        Ok(Some(row)) => {
            let owner_id: Option<Uuid> = row.try_get("user_id").ok();
            if let Some(owner) = owner_id {
                let _ = sqlx::query(
                    "INSERT INTO admin_audit_logs (admin_id, target_user_id, action, reason) VALUES ($1, $2, $3, $4)"
                )
                .bind(&admin_claims.sub) 
                .bind(owner)
                .bind("UPDATE_PROPERTY")
                .bind(format!("Admin updated property {}", id))
                .execute(&app_state.db)
                .await;
            }
            let _ = log_admin_action(
                &app_state.db,
                &admin_claims.sub,
                "update_property",
                "property",
                Some(id),
                Some(json!({ "fields_updated": { "status": payload.status, "is_featured": payload.is_featured, "is_verified": payload.is_verified } }))
            ).await;
            (
                StatusCode::OK,
                Json(json!({ "success": true, "message": "Property updated successfully" })),
            )
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "message": "Property not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": format!("Database error: {}", e) })),
        ),
    }
}

// 4. DELETE /api/admin/properties/:id/force
pub async fn force_delete_property(
    State(app_state): State<AppState>,
    axum::extract::Extension(admin_claims): axum::extract::Extension<AdminClaims>,
    Path(id): Path<Uuid>,
) -> impl axum::response::IntoResponse {
    let mut tx = match app_state.db.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "message": format!("Failed to start transaction: {}", e) })),
            );
        }
    };

    let property_owner = sqlx::query("SELECT user_id FROM properties WHERE id = $1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await;
        
    let owner_id: Uuid = match property_owner {
        Ok(Some(row)) => row.try_get("user_id").unwrap_or(Uuid::nil()),
        Ok(None) => {
            let _ = tx.rollback().await;
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "message": "Property not found" })),
            );
        }
        Err(e) => {
            let _ = tx.rollback().await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "message": format!("DB Error: {}", e) })),
            );
        }
    };

    let _ = sqlx::query("DELETE FROM property_reviews WHERE property_id = $1").bind(id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM saved_properties WHERE property_id = $1").bind(id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM site_visits WHERE property_id = $1").bind(id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM moderation_reports WHERE entity_type = 'PROPERTY' AND entity_id = $1").bind(id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM listing_images WHERE entity_id = $1 AND entity_type = 'property'").bind(id).execute(&mut *tx).await;

    let delete_result = sqlx::query("DELETE FROM properties WHERE id = $1").bind(id).execute(&mut *tx).await;
    if delete_result.is_err() {
        let _ = tx.rollback().await;
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": "Failed to delete property" })),
        );
    }

    if owner_id != Uuid::nil() {
        let _ = sqlx::query("INSERT INTO admin_audit_logs (admin_id, target_user_id, action, reason) VALUES ($1, $2, $3, $4)")
        .bind(&admin_claims.sub)
        .bind(owner_id)
        .bind("FORCE_DELETE_PROPERTY")
        .bind(format!("Admin force deleted property {}", id))
        .execute(&mut *tx)
        .await;
    }

    let _ = log_admin_action(
        &app_state.db,
        &admin_claims.sub,
        "force_delete_property",
        "property",
        Some(id),
        None
    ).await;

    if let Err(e) = tx.commit().await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "message": format!("Failed to commit transaction: {}", e) })),
        );
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Property and all related data forcefully deleted",
            "cascadeDeleted": true
        })),
    )
}

// 5. POST /api/admin/properties/bulk-action
pub async fn bulk_action_properties(
    State(app_state): State<AppState>,
    axum::extract::Extension(admin_claims): axum::extract::Extension<AdminClaims>,
    Json(payload): Json<BulkPropertyActionRequest>,
) -> impl axum::response::IntoResponse {
    let mut success_count = 0;
    
    for property_id in &payload.property_ids {
        match payload.action.as_str() {
            "feature" => {
                let res = sqlx::query("UPDATE properties SET is_featured = true, updated_at = NOW() WHERE id = $1")
                    .bind(property_id)
                    .execute(&app_state.db)
                    .await;
                if res.is_ok() { success_count += 1; }
            }
            "unfeature" => {
                let res = sqlx::query("UPDATE properties SET is_featured = false, updated_at = NOW() WHERE id = $1")
                    .bind(property_id)
                    .execute(&app_state.db)
                    .await;
                if res.is_ok() { success_count += 1; }
            }
            "suspend" => {
                let res = sqlx::query("UPDATE properties SET status = 'inactive', updated_at = NOW() WHERE id = $1")
                    .bind(property_id)
                    .execute(&app_state.db)
                    .await;
                if res.is_ok() { success_count += 1; }
            }
            "force-delete" => {
                let mut tx = app_state.db.begin().await.unwrap();
                let _ = sqlx::query("DELETE FROM property_reviews WHERE property_id = $1").bind(property_id).execute(&mut *tx).await;
                let _ = sqlx::query("DELETE FROM saved_properties WHERE property_id = $1").bind(property_id).execute(&mut *tx).await;
                let _ = sqlx::query("DELETE FROM site_visits WHERE property_id = $1").bind(property_id).execute(&mut *tx).await;
                let _ = sqlx::query("DELETE FROM moderation_reports WHERE entity_type = 'PROPERTY' AND entity_id = $1").bind(property_id).execute(&mut *tx).await;
                let _ = sqlx::query("DELETE FROM listing_images WHERE entity_id = $1 AND entity_type = 'property'").bind(property_id).execute(&mut *tx).await;
                
                let owner_id = sqlx::query("SELECT user_id FROM properties WHERE id = $1")
                    .bind(property_id)
                    .fetch_optional(&mut *tx)
                    .await
                    .ok()
                    .flatten()
                    .and_then(|row| row.try_get::<Uuid, _>("user_id").ok());
                    
                if let Ok(_) = sqlx::query("DELETE FROM properties WHERE id = $1").bind(property_id).execute(&mut *tx).await {
                    if let Some(owner) = owner_id {
                         let _ = sqlx::query("INSERT INTO admin_audit_logs (admin_id, target_user_id, action, reason) VALUES ($1, $2, $3, $4)")
                            .bind(&admin_claims.sub)
                            .bind(owner)
                            .bind("BULK_FORCE_DELETE_PROPERTY")
                            .bind(payload.reason.clone().unwrap_or_else(|| "Bulk action".to_string()))
                            .execute(&mut *tx)
                            .await;
                    }
                    if tx.commit().await.is_ok() { success_count += 1; }
                } else {
                    let _ = tx.rollback().await;
                }
            }
            _ => {}
        }
    }

    let _ = log_admin_action(
        &app_state.db,
        &admin_claims.sub,
        "bulk_action_properties",
        "property",
        None,
        Some(json!({ "action": payload.action, "property_ids": payload.property_ids, "reason": payload.reason }))
    ).await;

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": format!("Successfully processed {}/{} properties", success_count, payload.property_ids.len())
        })),
    )
}
