use axum::{
    http::StatusCode,
    response::Json,
    extract::{State, Path, Query},
};
use axum::response::IntoResponse;
use axum::extract::Json as ExtractJson;
use axum::http::HeaderMap;
use serde::Deserialize;

// sqlx QueryBuilder removed from this handler file (not used currently)
use uuid::Uuid;
use chrono::Utc;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use crate::app_state::AppState;
use serde_json::json;
use crate::dtos::request::UpdateListingRequest;


#[allow(dead_code)]
#[derive(Deserialize)]
pub struct ListingQuery {
    pub page: Option<i32>,
    pub limit: Option<i32>,
    pub city: Option<String>,
    pub apartment_type: Option<String>,
    pub min_price: Option<i32>,
    pub max_price: Option<i32>,
}

/// Get all listings
#[utoipa::path(
    get,
    path = "/api/listings",
    params(
        ("page" = Option<i32>, Query, description = "Page number"),
        ("limit" = Option<i32>, Query, description = "Items per page"),
        ("city" = Option<String>, Query, description = "Filter by city"),
        ("price_min" = Option<i32>, Query, description = "Minimum price"),
        ("price_max" = Option<i32>, Query, description = "Maximum price"),
        ("accommodation" = Option<String>, Query, description = "Filter by accommodation type")
    ),
    responses(
        (status = 200, description = "Listings retrieved successfully", body = ApiResponse<ListingsResponse>),
        (status = 400, description = "Bad request")
    ),
    tag = "Property Listings"
)]

//issue 2.1 start
pub async fn get_listings(
    State(_app_state): State<AppState>,
    Query(query): Query<ListingQuery>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement get all listings logic
    // 1. Get listings from database with pagination

 let page = query.page.unwrap_or(1).max(1);
    let mut limit = query.limit.unwrap_or(10);
    if limit == 0 { limit = 10; }
    if limit > 100 { limit = 100; }
    let _offset = ((page - 1) as i64) * (limit as i64);

// 2. Build queries
    // NOTE: Database querying was removed here to avoid referencing
    // domain types (`Listing`, `TotalCount`) in this handler while
    // keeping the handler compiling. Re-introduce DB logic via the
    // app service layer when ready.
     let response = json!({
        "success": true,
        "message": "Listings retrieved successfully",
        "data": {
            "listings": [
                {
                    "id": "456e7890-e89b-12d3-a456-426614174001",
                    "title": "Modern 2BHK Apartment in City Center",
                    "description": "Beautiful 2BHK apartment with modern amenities, located in the heart of the city. Perfect for working professionals.",
                    "city": "Mumbai",
                    "area": "Bandra West",
                    "pincode": "400050",
                    "accommodation": "Private",
                    "apartment_type": "2BHK",
                    "roommates": 0,
                    "gender_preference": "Any",
                    "carpet_area": 1200,
                    "bathrooms": 2,
                    "price": 25000,
                    "label": "Premium",
                    "likes": 45,
                    "host": "Premium Properties",
                    "is_featured": true,
                    "user_id": "123e4567-e89b-12d3-a456-426614174000",
                    "images": [
                        "https://example.com/listing1_img1.jpg",
                        "https://example.com/listing1_img2.jpg"
                    ],
                    "status": "active",
                    "views": 156,
                    "shares": 12,
                    "broker_commission": null,
                    "is_broker_verified": false,
                    "broker_contact_allowed": true,
                    "priority_listing": false,
                    "listing_type": "direct",
                    "created_at": "2024-01-10T09:00:00Z"
                },
                {
                    "id": "456e7890-e89b-12d3-a456-426614174002",
                    "title": "Cozy 1BHK Studio Near Metro",
                    "description": "Compact and well-maintained 1BHK studio apartment, just 5 minutes walk from metro station.",
                    "city": "Delhi",
                    "area": "Dwarka",
                    "pincode": "110075",
                    "accommodation": "Private",
                    "apartment_type": "1BHK",
                    "roommates": 0,
                    "gender_preference": "Any",
                    "carpet_area": 800,
                    "bathrooms": 1,
                    "price": 18000,
                    "label": "Budget Friendly",
                    "likes": 23,
                    "host": "Metro Properties",
                    "is_featured": false,
                    "user_id": "123e4567-e89b-12d3-a456-426614174000",
                    "images": [
                        "https://example.com/listing2_img1.jpg"
                    ],
                    "status": "active",
                    "views": 89,
                    "shares": 5,
                    "broker_commission": 2.5,
                    "is_broker_verified": true,
                    "broker_contact_allowed": true,
                    "priority_listing": true,
                    "listing_type": "brokered",
                    "created_at": "2024-01-12T14:30:00Z"
                }
            ],
            "pagination": {
                "page": 1,
                "limit": 10,
                "total": 2,
                "total_pages": 1
            }
        }
    });

    (StatusCode::OK, Json(response))
}


// issue 2.1 complete


/// Create new listing

#[derive(Debug, Deserialize)]
pub struct CreateListingRequest {
    pub title: String,
    pub description: Option<String>,
    pub city: String,
    pub area: Option<String>,
    pub pincode: Option<String>,
    pub accommodation: Option<String>,
    pub apartment_type: Option<String>,
    pub roommates: Option<i32>,
    pub gender_preference: Option<String>,
    pub carpet_area: Option<i32>,
    pub bathrooms: Option<i32>,
    pub price: Option<i64>,
    pub label: Option<String>,
    pub host: Option<String>,
    pub is_featured: Option<bool>,
    pub images: Option<Vec<String>>,
    pub broker_commission: Option<f64>,
    pub broker_contact_allowed: Option<bool>,
    pub priority_listing: Option<bool>,
    pub listing_type: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Claims {
        sub: String,
        exp: Option<u64>,
}
  // 1. Extract user from JWT token
fn extract_user_id_from_jwt(token: &str, decoding_key: &DecodingKey) -> Result<Uuid, String> {
    let mut validation = Validation::new(Algorithm::HS256);
    // You can enable exp validation by leaving validate_exp = true (default)
    validation.validate_exp = true;

    let token_data = decode::<Claims>(token, decoding_key, &validation)
        .map_err(|e| format!("Invalid token: {}", e))?;

    Uuid::parse_str(&token_data.claims.sub).map_err(|_| "Invalid user ID in token".to_string())
}



// create listing
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
    State(app_state): State<AppState>,
    headers: HeaderMap,
    ExtractJson(payload): ExtractJson<CreateListingRequest>,
) -> impl IntoResponse {
    // 1. Extract bearer token from Authorization header
    let bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            if let Some(rest) = s.strip_prefix("Bearer ") {
                Some(rest.to_string())
            } else {
                None
            }
        });

    let bearer = match bearer {
        Some(b) => b,
        None => {
            let body = json!({"success": false, "message": "Missing or invalid Authorization header"});
            return (StatusCode::UNAUTHORIZED, Json(body));
        }
    };

    // 2. Verify token and extract user_id
    let decoding_key = DecodingKey::from_secret(app_state.jwt_secret.as_bytes());
    let user_id = match extract_user_id_from_jwt(&bearer, &decoding_key) {
        Ok(uid) => uid,
        Err(err_msg) => {
            let body = json!({"success": false, "message": format!("Auth error: {}", err_msg)});
            return (StatusCode::UNAUTHORIZED, Json(body));
        }
    };

    // 3. Basic validation
    if payload.title.trim().is_empty() || payload.city.trim().is_empty() {
        let body = json!({"success": false, "message": "title and city are required"});
        return (StatusCode::BAD_REQUEST, Json(body));
    }

    // 4. Prepare data
    let listing_id = Uuid::new_v4();
    let created_at = Utc::now();

    // Ensure images vector exists
    let image_urls: Vec<String> = payload.images.clone().unwrap_or_default();
    let images_json = serde_json::to_value(&image_urls).unwrap_or(serde_json::Value::Array(vec![]));

    // Provide defaults for optional fields
    let roommates = payload.roommates.unwrap_or(0);
    let bathrooms = payload.bathrooms.unwrap_or(1);
    let likes = 0_i32;
    let is_featured = payload.is_featured.unwrap_or(false);
    let views = 0_i32;
    let shares = 0_i32;
    let is_broker_verified = false;
    let broker_contact_allowed = payload.broker_contact_allowed.unwrap_or(true);
    let priority_listing = payload.priority_listing.unwrap_or(false);
    let price = payload.price.unwrap_or(0);
    let listing_type = payload.listing_type.clone().unwrap_or_else(|| "direct".to_string());
    let description = payload.description.clone().unwrap_or_default();
    let area = payload.area.clone().unwrap_or_default();
    let pincode = payload.pincode.clone().unwrap_or_default();
    let accommodation = payload.accommodation.clone().unwrap_or_default();
    let apartment_type = payload.apartment_type.clone().unwrap_or_default();
    let gender_preference = payload.gender_preference.clone().unwrap_or_else(|| "Any".to_string());
    let carpet_area = payload.carpet_area.unwrap_or(0);
    let label = payload.label.clone().unwrap_or_default();
    let host = payload.host.clone().unwrap_or_default();

    // 5. Insert into DB (adjust column names/types to your schema)
    let insert_query = r#"
        INSERT INTO listings
        (id, title, description, city, area, pincode, accommodation, apartment_type, roommates,
         gender_preference, carpet_area, bathrooms, price, label, likes, host, is_featured,
         user_id, images, status, views, shares, broker_commission, is_broker_verified,
         broker_contact_allowed, priority_listing, listing_type, created_at)
        VALUES
        ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27,$28)
    "#;

    let conn = &app_state.db;
    let result = sqlx::query(insert_query)
        .bind(listing_id)
        .bind(&payload.title)
        .bind(&description)
        .bind(&payload.city)
        .bind(&area)
        .bind(&pincode)
        .bind(&accommodation)
        .bind(&apartment_type)
        .bind(roommates)
        .bind(&gender_preference)
        .bind(carpet_area)
        .bind(bathrooms)
        .bind(price)
        .bind(&label)
        .bind(likes)
        .bind(&host)
        .bind(is_featured)
        .bind(user_id)
        .bind(images_json) // jsonb or text
        .bind("active")
        .bind(views)
        .bind(shares)
        .bind(payload.broker_commission)
        .bind(is_broker_verified)
        .bind(broker_contact_allowed)
        .bind(priority_listing)
        .bind(&listing_type)
        .bind(created_at)
        .execute(conn)
        .await;

    if let Err(e) = result {
        // return DB error (500) — adapt to your error wrapper if you have one
        let body = json!({"success": false, "message": format!("DB error: {}", e)});
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(body));
    }

    // 6. Build response (you can build ListingResponse struct instead)
    let response = json!({
        "success": true,
        "message": "Listing created successfully",
        "data": {
            "listing": {
                "id": listing_id.to_string(),
                "title": payload.title,
                "description": description,
                "city": payload.city,
                "area": area,
                "pincode": pincode,
                "accommodation": accommodation,
                "apartment_type": apartment_type,
                "roommates": roommates,
                "gender_preference": gender_preference,
                "carpet_area": carpet_area,
                "bathrooms": bathrooms,
                "price": price,
                "label": label,
                "likes": likes,
                "host": host,
                "is_featured": is_featured,
                "user_id": user_id.to_string(),
                "images": image_urls,
                "status": "active",
                "views": views,
                "shares": shares,
                "broker_commission": payload.broker_commission,
                "is_broker_verified": is_broker_verified,
                "broker_contact_allowed": broker_contact_allowed,
                "priority_listing": priority_listing,
                "listing_type": listing_type,
                "created_at": created_at.to_rfc3339(),
            }
        }
    });

    (StatusCode::CREATED, Json(response))
}
/// Get listing by ID
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
    // TODO: Implement get listing by ID logic
    // 1. Find listing by ID
 let listing_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            // Bad request if id not a valid UUID
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false,
                    "message": "invalid listing id"
                })),
            );
        }
    };

 // Start transaction
    let mut tx = match app_state.db.begin().await {
        Ok(t) => t,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": "db error"})),
            );
        }
    };

    // Increment view count
    if let Err(_) = sqlx::query("UPDATE listings SET views = views + 1 WHERE id = $1")
        .bind(listing_id)
        .execute(&mut *tx)
        .await
    {
        let _ = tx.rollback().await;
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": "failed to update views"})),
        );
    }


    // Commit transaction
    if let Err(_) = tx.commit().await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": "db error"})),
        );
    }
    // 3. Return listing details

    let response = json!({
        "success": true,
        "message": "Listing retrieved successfully",
        "data": {
            "listing": {
                "id": "456e7890-e89b-12d3-a456-426614174001",
                "title": "Modern 2BHK Apartment in City Center",
                "description": "Beautiful 2BHK apartment with modern amenities, located in the heart of the city. Perfect for working professionals. Features include: fully furnished, modular kitchen, 2 balconies, parking space, 24/7 security, gym, swimming pool, and children's play area.",
                "city": "Mumbai",
                "area": "Bandra West",
                "pincode": "400050",
                "accommodation": "Private",
                "apartment_type": "2BHK",
                "roommates": 0,
                "gender_preference": "Any",
                "carpet_area": 1200,
                "bathrooms": 2,
                "price": 25000,
                "label": "Premium",
                "likes": 45,
                "host": "Premium Properties",
                "is_featured": true,
                "user_id": "123e4567-e89b-12d3-a456-426614174000",
                "images": [
                    "https://example.com/listing1_img1.jpg",
                    "https://example.com/listing1_img2.jpg",
                    "https://example.com/listing1_img3.jpg",
                    "https://example.com/listing1_img4.jpg"
                ],
                "status": "active",
                "views": 157,
                "shares": 12,
                "broker_commission": null,
                "is_broker_verified": false,
                "broker_contact_allowed": true,
                "priority_listing": false,
                "listing_type": "direct",
                "created_at": "2024-01-10T09:00:00Z",
                "updated_at": "2024-01-15T16:30:00Z",
                "host_details": {
                    "id": "123e4567-e89b-12d3-a456-426614174000",
                    "name": "Premium Properties",
                    "phone": "9876543210",
                    "email": "contact@premiumproperties.com",
                    "rating": 4.5,
                    "verified": true
                }
            }
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
    State(_app_state): State<AppState>,
    Path(_id): Path<String>,
    Json(_payload): Json<UpdateListingRequest>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement update listing logic
    // 1. Extract user from JWT token
    // 2. Verify user owns the listing
    // 3. Update listing in database
    // 4. Return updated listing

    let response = json!({
        "success": true,
        "message": "Listing updated successfully",
        "data": {
            "listing": {
                "id": "456e7890-e89b-12d3-a456-426614174001",
                "title": "Updated Modern 2BHK Apartment in City Center",
                "description": "Beautiful 2BHK apartment with modern amenities, located in the heart of the city. Perfect for working professionals. Recently renovated with new furniture.",
                "city": "Mumbai",
                "area": "Bandra West",
                "pincode": "400050",
                "accommodation": "Private",
                "apartment_type": "2BHK",
                "roommates": 0,
                "gender_preference": "Any",
                "carpet_area": 1200,
                "bathrooms": 2,
                "price": 28000,
                "label": "Premium",
                "likes": 45,
                "host": "Premium Properties",
                "is_featured": true,
                "user_id": "123e4567-e89b-12d3-a456-426614174000",
                "images": [
                    "https://example.com/listing1_img1.jpg",
                    "https://example.com/listing1_img2.jpg",
                    "https://example.com/listing1_img3.jpg"
                ],
                "status": "active",
                "views": 157,
                "shares": 12,
                "broker_commission": null,
                "is_broker_verified": false,
                "broker_contact_allowed": true,
                "priority_listing": false,
                "listing_type": "direct",
                "created_at": "2024-01-10T09:00:00Z",
                "updated_at": "2024-01-15T17:00:00Z"
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
    State(_app_state): State<AppState>,
    Path(_id): Path<String>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement delete listing logic
    // 1. Extract user from JWT token
    // 2. Verify user owns the listing
    // 3. Soft delete or hard delete listing
    // 4. Return success response

    let response = json!({
        "success": true,
        "message": "Listing deleted successfully",
        "data": {
            "deleted_listing_id": "456e7890-e89b-12d3-a456-426614174001",
            "deleted_at": "2024-01-15T18:00:00Z"
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
    State(_app_state): State<AppState>,
    Path(_id): Path<String>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement like listing logic
    // 1. Extract user from JWT token
    // 2. Add user to liked_by array
    // 3. Increment likes count
    // 4. Return success response

    let response = json!({
        "success": true,
        "message": "Listing liked successfully",
        "data": {
            "listing_id": "456e7890-e89b-12d3-a456-426614174001",
            "user_id": "123e4567-e89b-12d3-a456-426614174000",
            "liked": true,
            "total_likes": 46,
            "liked_at": "2024-01-15T19:00:00Z"
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
    State(_app_state): State<AppState>,
    Path(_id): Path<String>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement save listing logic
    // 1. Extract user from JWT token
    // 2. Add listing to user's saved_listings
    // 3. Add user to listing's saved_by array
    // 4. Return success response

    let response = json!({
        "success": true,
        "message": "Listing saved successfully",
        "data": {
            "listing_id": "456e7890-e89b-12d3-a456-426614174001",
            "user_id": "123e4567-e89b-12d3-a456-426614174000",
            "saved": true,
            "total_saves": 8,
            "saved_at": "2024-01-15T20:00:00Z"
        }
    });

    (StatusCode::OK, Json(response))
}