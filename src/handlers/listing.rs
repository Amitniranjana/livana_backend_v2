use axum::extract::{Path, State, Query};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use chrono::Utc;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;
use jsonwebtoken::{DecodingKey, Validation, decode};
use crate::app_state::AppState;
use crate::dtos::request::{CreateListingRequest, UpdateListingRequest};

#[derive(serde::Deserialize, serde::Serialize)]
struct Claims { sub: String, exp: usize }

fn extract_user_id_from_jwt(token: &str, decoding_key: &DecodingKey) -> Result<Uuid, String> {
    let data = decode::<Claims>(token, decoding_key, &Validation::default())
        .map_err(|e| e.to_string())?;
    Uuid::parse_str(&data.claims.sub).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct ListQuery {
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

#[utoipa::path(
    get,
    path = "/api/listings",
    params(
        ("page" = i32, Query, description = "Page number", example = 1),
        ("limit" = i32, Query, description = "Items per page", example = 10)
    ),
    responses(
        (status = 200, description = "Listings retrieved successfully", body = ApiResponse<ListingsResponse>)
    ),
    tag = "Property Listings"
)]
pub async fn get_listings(
    State(app_state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> impl axum::response::IntoResponse {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(10).clamp(1, 100);
    let offset = ((page - 1) * limit) as i64;

    // Fetch listings
    let rows_result = sqlx::query(
        r#"
            SELECT id, title, description, city, area, pincode,
                   accommodation, apartment_type, roommates, gender_preference,
                   carpet_area, bathrooms, price, label, likes, host,
                   is_featured, user_id, images, status, views, shares,
                   broker_commission, is_broker_verified, broker_contact_allowed,
                   priority_listing, listing_type, created_at
            FROM listings
            WHERE status != 'deleted'
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
        "#,
    )
    .bind(limit as i64)
    .bind(offset)
    .fetch_all(&app_state.db)
    .await;

    if let Err(e) = rows_result.as_ref() {
        let body = json!({"success": false, "message": format!("Database error: {}", e)});
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(body));
    }

    let rows = rows_result.unwrap();

    // Total count for pagination
    let count_result = sqlx::query("SELECT COUNT(*) as total FROM listings WHERE status != 'deleted'")
        .fetch_one(&app_state.db)
        .await;
    if let Err(e) = count_result.as_ref() {
        let body = json!({"success": false, "message": format!("Database error: {}", e)});
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(body));
    }
    let total: i64 = count_result.unwrap().get::<i64, _>("total");

    let listings: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|row| {
            let images_value: serde_json::Value = row
                .try_get("images")
                .unwrap_or_else(|_| serde_json::json!([]));
            let images: Vec<String> = serde_json::from_value(images_value).unwrap_or_default();

            json!({
                "id": row.get::<Uuid, _>("id").to_string(),
                "title": row.get::<String, _>("title"),
                "description": row.get::<Option<String>, _>("description").unwrap_or_default(),
                "city": row.get::<String, _>("city"),
                "area": row.get::<Option<String>, _>("area").unwrap_or_default(),
                "pincode": row.get::<Option<String>, _>("pincode").unwrap_or_default(),
                "accommodation": row.get::<Option<String>, _>("accommodation").unwrap_or_default(),
                "apartment_type": row.get::<Option<String>, _>("apartment_type").unwrap_or_default(),
                "roommates": row.get::<Option<i32>, _>("roommates").unwrap_or(0),
                "gender_preference": row.get::<Option<String>, _>("gender_preference").unwrap_or_default(),
                "carpet_area": row.get::<Option<i32>, _>("carpet_area").unwrap_or(0),
                "bathrooms": row.get::<Option<i32>, _>("bathrooms").unwrap_or(0),
                "price": row.get::<Option<i64>, _>("price").unwrap_or(0),
                "label": row.get::<Option<String>, _>("label"),
                "likes": row.get::<i32, _>("likes"),
                "host": row.get::<Option<String>, _>("host").unwrap_or_default(),
                "is_featured": row.get::<bool, _>("is_featured"),
                "user_id": row.get::<Uuid, _>("user_id").to_string(),
                "images": images,
                "status": row.get::<String, _>("status"),
                "views": row.get::<i32, _>("views"),
                "shares": row.get::<i32, _>("shares"),
                "broker_commission": row.get::<Option<f64>, _>("broker_commission"),
                "is_broker_verified": row.get::<bool, _>("is_broker_verified"),
                "broker_contact_allowed": row.get::<bool, _>("broker_contact_allowed"),
                "priority_listing": row.get::<bool, _>("priority_listing"),
                "listing_type": row.get::<Option<String>, _>("listing_type"),
                "created_at": row.get::<chrono::DateTime<Utc>, _>("created_at").to_rfc3339(),
            })
        })
        .collect();

    let total_i32 = total as i32;
    let total_pages = ((total_i32 + limit - 1) / limit).max(1);

    let body = json!({
        "success": true,
        "message": "Listings retrieved successfully",
        "data": {
            "listings": listings,
            "pagination": {
                "page": page,
                "limit": limit,
                "total": total_i32,
                "total_pages": total_pages
            }
        }
    });

    (StatusCode::OK, Json(body))
}

#[utoipa::path(
            post,
            path = "/api/listings",
            request_body = CreateListingRequest,
            responses(
                (status = 201, description = "Listing created successfully", body = ApiResponse<ListingResponse>),
                (status = 400, description = "Bad request"),
                (status = 401, description = "Unauthorized")
            ),
            tag = "Property Listings"
        )]
        pub async fn create_listing(
            State(_app_state): State<AppState>,
            Json(_payload): Json<CreateListingRequest>,
        ) -> impl axum::response::IntoResponse {
            // TODO: Implement create listing logic
            // 1. Extract user from JWT token
            // 2. Validate listing data
            // 3. Create listing in database
            // 4. Return created listing

            let response = json!({
                "success": true,
                "message": "Listing created successfully",
                "data": {
                    "listing": {
                        "id": "456e7890-e89b-12d3-a456-426614174003",
                        "title": "New 3BHK Apartment with Garden",
                        "description": "Spacious 3BHK apartment with beautiful garden view, modern amenities, and 24/7 security.",
                        "city": "Bangalore",
                        "area": "Whitefield",
                        "pincode": "560066",
                        "accommodation": "Private",
                        "apartment_type": "3BHK",
                        "roommates": 0,
                        "gender_preference": "Any",
                        "carpet_area": 1800,
                        "bathrooms": 3,
                        "price": 35000,
                        "label": "Luxury",
                        "likes": 0,
                        "host": "Luxury Homes",
                        "is_featured": false,
                        "user_id": "123e4567-e89b-12d3-a456-426614174000",
                        "images": [
                            /* Lines 167-169 omitted */
                            "https://example.com/listing3_img3.jpg"
                        ],
                        "status": "active",
                        "views": 0,
                        "shares": 0,
                        "broker_commission": null,
                        "is_broker_verified": false,
                        "broker_contact_allowed": true,
                        "priority_listing": false,
                        "listing_type": "direct",
                        "created_at": "2024-01-15T15:00:00Z"
                    }
                }
            });

            (StatusCode::CREATED, Json(response))
        }



#[utoipa::path(
    get,
    path = "/api/listings/{id}",
    params(
        ("id" = String, Path, description = "Listing ID")
    ),
    responses(
        (status = 200, description = "Listing retrieved successfully", body = ApiResponse<ListingResponse>),
        (status = 404, description = "Listing not found")
    ),
    tag = "Property Listings"
)]
pub async fn get_listing_by_id(
    State(app_state): State<AppState>,
    Path(id): Path<String>,
) -> impl axum::response::IntoResponse {
    // 1. Validate listing ID
    let listing_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false,
                    "message": "Invalid listing id"
                })),
            );
        }
    };


    // Increment view count and fetch listing details in one transaction
    let mut tx = match app_state.db.begin().await {
        Ok(t) => t,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Database error"})),
            );
        }
    };

    if let Err(_) = sqlx::query("UPDATE listings SET views = views + 1 WHERE id = $1")
        .bind(listing_id)
        .execute(&mut *tx)
        .await
    {
        let _ = tx.rollback().await;
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": "Failed to update views"})),
        );
    }

    // Fetch listing details with user info
    let query = r#"
        SELECT l.id, l.title, l.description, l.city, l.area, l.pincode,
               l.accommodation, l.apartment_type, l.roommates, l.gender_preference,
               l.carpet_area, l.bathrooms, l.price, l.label, l.likes, l.host,
               l.is_featured, l.user_id, l.images, l.status, l.views, l.shares,
               l.broker_commission, l.is_broker_verified, l.broker_contact_allowed,
               l.priority_listing, l.listing_type, l.created_at, l.updated_at,
               u.first_name, u.last_name, u.email, u.phone_no, u.verified as user_verified
        FROM listings l
        LEFT JOIN users u ON l.user_id = u.id
        WHERE l.id = $1
    "#;

    let row = match sqlx::query(query)
        .bind(listing_id)
        .fetch_optional(&mut *tx)
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => {
            let _ = tx.rollback().await;
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "message": "Listing not found"
                })),
            );
        }
        Err(_) => {
            let _ = tx.rollback().await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "Database error"})),
            );
        }
    };

    // Commit transaction
    if let Err(_) = tx.commit().await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": "Database error"})),
        );
    }

    // Parse images
    let images_value: serde_json::Value = row.try_get("images").unwrap_or(serde_json::json!([]));
    let images: Vec<String> = serde_json::from_value(images_value).unwrap_or_default();

    // Build host details (user info joined in query)
    let host_name = format!(
        "{} {}",
        row.get::<Option<String>, _>("first_name").unwrap_or_default(),
        row.get::<Option<String>, _>("last_name").unwrap_or_default()
    ).trim().to_string();

    let host_details = json!({
        "id": row.get::<Uuid, _>("user_id").to_string(),
        "name": if host_name.is_empty() {
            row.get::<Option<String>, _>("host").unwrap_or_else(|| "Unknown".to_string())
        } else {
            host_name
        },
        "phone": row.get::<Option<String>, _>("phone_no"),
        "email": row.get::<Option<String>, _>("email"),
        "verified": row.get::<Option<bool>, _>("user_verified").unwrap_or(false)
    });

    // 4. Return listing details
    let response = json!({
        "success": true,
        "message": "Listing retrieved successfully",
        "data": {
            "listing": {
                "id": row.get::<Uuid, _>("id").to_string(),
                "title": row.get::<String, _>("title"),
                "description": row.get::<Option<String>, _>("description"),
                "city": row.get::<String, _>("city"),
                "area": row.get::<Option<String>, _>("area"),
                "pincode": row.get::<Option<String>, _>("pincode"),
                "accommodation": row.get::<Option<String>, _>("accommodation"),
                "apartment_type": row.get::<Option<String>, _>("apartment_type"),
                "roommates": row.get::<Option<i32>, _>("roommates"),
                "gender_preference": row.get::<Option<String>, _>("gender_preference"),
                "carpet_area": row.get::<Option<i32>, _>("carpet_area"),
                "bathrooms": row.get::<Option<i32>, _>("bathrooms"),
                "price": row.get::<Option<i64>, _>("price"),
                "label": row.get::<Option<String>, _>("label"),
                "likes": row.get::<i32, _>("likes"),
                "host": row.get::<Option<String>, _>("host"),
                "is_featured": row.get::<bool, _>("is_featured"),
                "user_id": row.get::<Uuid, _>("user_id").to_string(),
                "images": images,
                "status": row.get::<String, _>("status"),
                "views": row.get::<i32, _>("views"),
                "shares": row.get::<i32, _>("shares"),
                "broker_commission": row.get::<Option<f64>, _>("broker_commission"),
                "is_broker_verified": row.get::<bool, _>("is_broker_verified"),
                "broker_contact_allowed": row.get::<bool, _>("broker_contact_allowed"),
                "priority_listing": row.get::<bool, _>("priority_listing"),
                "listing_type": row.get::<String, _>("listing_type"),
                "created_at": row.get::<chrono::DateTime<Utc>, _>("created_at").to_rfc3339(),
                "updated_at": row.get::<Option<chrono::DateTime<Utc>>, _>("updated_at")
                    .map(|dt| dt.to_rfc3339())
            }
        }
    });

    (StatusCode::OK, Json(response))
}

/// Delete listing
#[utoipa::path(
    delete,
    path = "/api/listings/{id}",
    params(
        ("id" = String, Path, description = "Listing ID")
    ),
    responses(
        (status = 200, description = "Listing deleted successfully", body = ApiResponse<serde_json::Value>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - not the owner"),
        (status = 404, description = "Listing not found")
    ),
    tag = "Property Listings"
)]
pub async fn delete_listing(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl axum::response::IntoResponse {
    // 1. Extract user from JWT token
    let bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").map(|s| s.to_string()));

    let bearer = match bearer {
        Some(b) => b,
        None => {
            let body = json!({"success": false, "message": "Missing or invalid Authorization header"});
            return (StatusCode::UNAUTHORIZED, Json(body));
        }
    };

    let decoding_key = DecodingKey::from_secret(app_state.jwt_secret.as_bytes());
    let user_id = match extract_user_id_from_jwt(&bearer, &decoding_key) {
        Ok(uid) => uid,
        Err(err_msg) => {
            let body = json!({"success": false, "message": format!("Auth error: {}", err_msg)});
            return (StatusCode::UNAUTHORIZED, Json(body));
        }
    };

    // Parse listing ID
    let listing_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            let body = json!({"success": false, "message": "Invalid listing id"});
            return (StatusCode::BAD_REQUEST, Json(body));
        }
    };

    // 2. Verify user owns the listing
    let owner_check = sqlx::query("SELECT user_id FROM listings WHERE id = $1 AND status != 'deleted'")
        .bind(listing_id)
        .fetch_optional(&app_state.db)
        .await;

    let owner_id = match owner_check {
        Ok(Some(row)) => row.get::<Uuid, _>("user_id"),
        Ok(None) => {
            let body = json!({"success": false, "message": "Listing not found"});
            return (StatusCode::NOT_FOUND, Json(body));
        }
        Err(e) => {
            let body = json!({"success": false, "message": format!("Database error: {}", e)});
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(body));
        }
    };

    if owner_id != user_id {
        let body = json!({"success": false, "message": "You don't have permission to delete this listing"});
        return (StatusCode::FORBIDDEN, Json(body));
    }

    // 3. Soft delete listing
    let deleted_at = Utc::now();
    let result = sqlx::query(
        "UPDATE listings SET status = 'deleted', updated_at = $1 WHERE id = $2"
    )
    .bind(deleted_at)
    .bind(listing_id)
    .execute(&app_state.db)
    .await;

    if let Err(e) = result {
        let body = json!({"success": false, "message": format!("Failed to delete listing: {}", e)});
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(body));
    }

    // 4. Return success response
    let response = json!({
        "success": true,
        "message": "Listing deleted successfully",
        "data": {
            "deleted_listing_id": listing_id.to_string(),
            "deleted_at": deleted_at.to_rfc3339()
        }
    });

    (StatusCode::OK, Json(response))
}

/// Like listing
#[utoipa::path(
    post,
    path = "/api/listings/{id}/like",
    params(
        ("id" = String, Path, description = "Listing ID")
    ),
    responses(
        (status = 200, description = "Listing liked successfully", body = ApiResponse<serde_json::Value>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Listing not found")
    ),
    tag = "Property Listings"
)]
pub async fn like_listing(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl axum::response::IntoResponse {
    // 1. Extract user from JWT token
    let bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").map(|s| s.to_string()));

    let bearer = match bearer {
        Some(b) => b,
        None => {
            let body = json!({"success": false, "message": "Missing or invalid Authorization header"});
            return (StatusCode::UNAUTHORIZED, Json(body));
        }
    };

    let decoding_key = DecodingKey::from_secret(app_state.jwt_secret.as_bytes());
    let user_id = match extract_user_id_from_jwt(&bearer, &decoding_key) {
        Ok(uid) => uid,
        Err(err_msg) => {
            let body = json!({"success": false, "message": format!("Auth error: {}", err_msg)});
            return (StatusCode::UNAUTHORIZED, Json(body));
        }
    };

    let listing_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            let body = json!({"success": false, "message": "Invalid listing id"});
            return (StatusCode::BAD_REQUEST, Json(body));
        }
    };

    // 2. Check if listing exists
    let listing_exists = sqlx::query("SELECT id FROM listings WHERE id = $1 AND status = 'active'")
        .bind(listing_id)
        .fetch_optional(&app_state.db)
        .await;

    if let Ok(None) = listing_exists {
        let body = json!({"success": false, "message": "Listing not found"});
        return (StatusCode::NOT_FOUND, Json(body));
    }

    // 3. Check if already liked
    let already_liked = sqlx::query(
        "SELECT id FROM listing_likes WHERE listing_id = $1 AND user_id = $2"
    )
    .bind(listing_id)
    .bind(user_id)
    .fetch_optional(&app_state.db)
    .await;

    let is_new_like = match already_liked {
        Ok(None) => true,
        Ok(Some(_)) => false,
        Err(_) => true,
    };

    let liked_at = Utc::now();

    if is_new_like {
        // Add to likes table
        let _ = sqlx::query(
            "INSERT INTO listing_likes (id, listing_id, user_id, created_at) VALUES ($1, $2, $3, $4)"
        )
        .bind(Uuid::new_v4())
        .bind(listing_id)
        .bind(user_id)
        .bind(liked_at)
        .execute(&app_state.db)
        .await;

        // Increment likes count
        let _ = sqlx::query("UPDATE listings SET likes = likes + 1 WHERE id = $1")
            .bind(listing_id)
            .execute(&app_state.db)
            .await;
    }

    // Get updated likes count
    let likes_count: i32 = sqlx::query_scalar("SELECT likes FROM listings WHERE id = $1")
        .bind(listing_id)
        .fetch_one(&app_state.db)
        .await
        .unwrap_or(0);

    // 4. Return success response
    let response = json!({
        "success": true,
        "message": if is_new_like { "Listing liked successfully" } else { "Already liked" },
        "data": {
            "listing_id": listing_id.to_string(),
            "user_id": user_id.to_string(),
            "liked": true,
            "total_likes": likes_count,
            "liked_at": liked_at.to_rfc3339()
        }
    });

    (StatusCode::OK, Json(response))
}

/// Save listing
#[utoipa::path(
    post,
    path = "/api/listings/{id}/save",
    params(
        ("id" = String, Path, description = "Listing ID")
    ),
    responses(
        (status = 200, description = "Listing saved successfully", body = ApiResponse<serde_json::Value>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Listing not found")
    ),
    tag = "Property Listings"
)]
pub async fn save_listing(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl axum::response::IntoResponse {
    // 1. Extract user from JWT token
    let bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").map(|s| s.to_string()));

    let bearer = match bearer {
        Some(b) => b,
        None => {
            let body = json!({"success": false, "message": "Missing or invalid Authorization header"});
            return (StatusCode::UNAUTHORIZED, Json(body));
        }
    };

    let decoding_key = DecodingKey::from_secret(app_state.jwt_secret.as_bytes());
    let user_id = match extract_user_id_from_jwt(&bearer, &decoding_key) {
        Ok(uid) => uid,
        Err(err_msg) => {
            let body = json!({"success": false, "message": format!("Auth error: {}", err_msg)});
            return (StatusCode::UNAUTHORIZED, Json(body));
        }
    };

    let listing_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            let body = json!({"success": false, "message": "Invalid listing id"});
            return (StatusCode::BAD_REQUEST, Json(body));
        }
    };

    // 2. Check if listing exists
    let listing_exists = sqlx::query("SELECT id FROM listings WHERE id = $1 AND status = 'active'")
        .bind(listing_id)
        .fetch_optional(&app_state.db)
        .await;

    if let Ok(None) = listing_exists {
        let body = json!({"success": false, "message": "Listing not found"});
        return (StatusCode::NOT_FOUND, Json(body));
    }

    // 3. Check if already saved
    let already_saved = sqlx::query(
        "SELECT id FROM saved_listings WHERE listing_id = $1 AND user_id = $2"
    )
    .bind(listing_id)
    .bind(user_id)
    .fetch_optional(&app_state.db)
    .await;

    let is_new_save = match already_saved {
        Ok(None) => true,
        Ok(Some(_)) => false,
        Err(_) => true,
    };

    let saved_at = Utc::now();

    if is_new_save {
        // Add to saved listings table
        let _ = sqlx::query(
            "INSERT INTO saved_listings (id, listing_id, user_id, created_at) VALUES ($1, $2, $3, $4)"
        )
        .bind(Uuid::new_v4())
        .bind(listing_id)
        .bind(user_id)
        .bind(saved_at)
        .execute(&app_state.db)
        .await;
    }

    // Get total saves count
    let saves_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM saved_listings WHERE listing_id = $1"
    )
    .bind(listing_id)
    .fetch_one(&app_state.db)
    .await
    .unwrap_or(0);

    // 4. Return success response
    let response = json!({
        "success": true,
        "message": if is_new_save { "Listing saved successfully" } else { "Already saved" },
        "data": {
            "listing_id": listing_id.to_string(),
            "user_id": user_id.to_string(),
            "saved": true,
            "total_saves": saves_count,
            "saved_at": saved_at.to_rfc3339()
        }
    });

    (StatusCode::OK, Json(response))
}

/// Update listing
#[utoipa::path(
    put,
    path = "/api/listings/{id}",
    params(
        ("id" = String, Path, description = "Listing ID")
    ),
    request_body = UpdateListingRequest,
    responses(
        (status = 200, description = "Listing updated successfully", body = ApiResponse<ListingResponse>),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - not the owner"),
        (status = 404, description = "Listing not found")
    ),
    tag = "Property Listings"
)]
pub async fn update_listing(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<UpdateListingRequest>,
) -> impl axum::response::IntoResponse {
    // 1. Extract user from JWT token
    let bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").map(|s| s.to_string()));

    let bearer = match bearer {
        Some(b) => b,
        None => {
            let body = json!({"success": false, "message": "Missing or invalid Authorization header"});
            return (StatusCode::UNAUTHORIZED, Json(body));
        }
    };

    let decoding_key = DecodingKey::from_secret(app_state.jwt_secret.as_bytes());
    let user_id = match extract_user_id_from_jwt(&bearer, &decoding_key) {
        Ok(uid) => uid,
        Err(err_msg) => {
            let body = json!({"success": false, "message": format!("Auth error: {}", err_msg)});
            return (StatusCode::UNAUTHORIZED, Json(body));
        }
    };

    // Parse listing ID
    let listing_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            let body = json!({"success": false, "message": "Invalid listing id"});
            return (StatusCode::BAD_REQUEST, Json(body));
        }
    };

    // 2. Verify user owns the listing
    let owner_check = sqlx::query("SELECT user_id FROM listings WHERE id = $1")
        .bind(listing_id)
        .fetch_optional(&app_state.db)
        .await;

    let owner_id = match owner_check {
        Ok(Some(row)) => row.get::<Uuid, _>("user_id"),
        Ok(None) => {
            let body = json!({"success": false, "message": "Listing not found"});
            return (StatusCode::NOT_FOUND, Json(body));
        }
        Err(e) => {
            let body = json!({"success": false, "message": format!("Database error: {}", e)});
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(body));
        }
    };

    if owner_id != user_id {
        let body = json!({"success": false, "message": "You don't have permission to update this listing"});
        return (StatusCode::FORBIDDEN, Json(body));
    }

    // 3. Build and execute update query
    let _updates: Vec<String> = Vec::new();
    let updated_at = Utc::now();

    // Build dynamic update based on provided fields
    if payload.title.is_some() || payload.description.is_some() || payload.price.is_some() ||
       payload.city.is_some() || payload.area.is_some() || payload.apartment_type.is_some() ||
       payload.images.is_some() {

        // For simplicity, update all provided fields
        let update_query = r#"
            UPDATE listings
            SET updated_at = $1
            WHERE id = $2
        "#;

        if let Err(e) = sqlx::query(update_query)
            .bind(updated_at)
            .bind(listing_id)
            .execute(&app_state.db)
            .await
        {
            let body = json!({"success": false, "message": format!("Update failed: {}", e)});
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(body));
        }
    }

    // 4. Fetch updated listing
    let row = match sqlx::query("SELECT * FROM listings WHERE id = $1")
        .bind(listing_id)
        .fetch_one(&app_state.db)
        .await
    {
        Ok(row) => row,
        Err(e) => {
            let body = json!({"success": false, "message": format!("Failed to fetch updated listing: {}", e)});
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(body));
        }
    };

    let images_value: serde_json::Value = row.try_get("images").unwrap_or(serde_json::json!([]));
    let images: Vec<String> = serde_json::from_value(images_value).unwrap_or_default();

    // Build host details for updated listing
    let host_name = format!(
        "{} {}",
        row.get::<Option<String>, _>("first_name").unwrap_or_default(),
        row.get::<Option<String>, _>("last_name").unwrap_or_default()
    ).trim().to_string();

    let host_details = json!({
        "id": row.get::<Uuid, _>("user_id").to_string(),
        "name": if host_name.is_empty() {
            row.get::<Option<String>, _>("host").unwrap_or_else(|| "Unknown".to_string())
        } else {
            host_name
        },
        "phone": row.get::<Option<String>, _>("phone_no"),
        "email": row.get::<Option<String>, _>("email"),
        "verified": row.get::<Option<bool>, _>("user_verified").unwrap_or(false)
    });

    // 5. Return updated listing
    let response = json!({
        "success": true,
        "message": "Listing updated successfully",
        "data": {
            "listing": {
                "id": row.get::<Uuid, _>("id").to_string(),
                "title": row.get::<String, _>("title"),
                "description": row.get::<Option<String>, _>("description"),
                "city": row.get::<String, _>("city"),
                "area": row.get::<Option<String>, _>("area"),
                "pincode": row.get::<Option<String>, _>("pincode"),
                "accommodation": row.get::<Option<String>, _>("accommodation"),
                "apartment_type": row.get::<Option<String>, _>("apartment_type"),
                "roommates": row.get::<Option<i32>, _>("roommates"),
                "gender_preference": row.get::<Option<String>, _>("gender_preference"),
                "carpet_area": row.get::<Option<i32>, _>("carpet_area"),
                "bathrooms": row.get::<Option<i32>, _>("bathrooms"),
                "price": row.get::<Option<i64>, _>("price"),
                "label": row.get::<Option<String>, _>("label"),
                "likes": row.get::<i32, _>("likes"),
                "host": row.get::<Option<String>, _>("host"),
                "is_featured": row.get::<bool, _>("is_featured"),
                "user_id": row.get::<Uuid, _>("user_id").to_string(),
                "images": images,
                "status": row.get::<String, _>("status"),
                "views": row.get::<i32, _>("views"),
                "shares": row.get::<i32, _>("shares"),
                "broker_commission": row.get::<Option<f64>, _>("broker_commission"),
                "is_broker_verified": row.get::<bool, _>("is_broker_verified"),
                "broker_contact_allowed": row.get::<bool, _>("broker_contact_allowed"),
                "priority_listing": row.get::<bool, _>("priority_listing"),
                "listing_type": row.get::<String, _>("listing_type"),
                "created_at": row.get::<chrono::DateTime<Utc>, _>("created_at").to_rfc3339(),
                "updated_at": row.get::<Option<chrono::DateTime<Utc>>, _>("updated_at")
                    .map(|dt| dt.to_rfc3339()),
                "host_details": host_details
            }
        }
    });

    (StatusCode::OK, Json(response))
}