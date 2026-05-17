use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use jsonwebtoken::DecodingKey;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::dtos::request::{CreatePropertyRequest, ReportPropertyRequest, UpdatePropertyRequest};

// ---------------------------------------------------------------------------
// JWT Helper
// ---------------------------------------------------------------------------

fn extract_user_id_from_jwt(token: &str, decoding_key: &DecodingKey) -> Result<Uuid, String> {
    use jsonwebtoken::{Algorithm, Validation, decode};
    #[derive(serde::Deserialize)]
    struct Claims {
        sub: String,
    }
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = false;
    let data = decode::<Claims>(token, decoding_key, &validation).map_err(|e| e.to_string())?;
    Uuid::parse_str(&data.claims.sub).map_err(|e| e.to_string())
}

/// Extracts the authenticated user_id from Bearer token in headers.
/// Returns early with 401 if missing or invalid.
macro_rules! require_auth {
    ($headers:expr, $app_state:expr) => {{
        let bearer = $headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer ").map(|s| s.to_string()));
        match bearer {
            Some(b) => {
                let dk = DecodingKey::from_secret($app_state.jwt_secret.as_bytes());
                match extract_user_id_from_jwt(&b, &dk) {
                    Ok(uid) => uid,
                    Err(e) => {
                        return (
                            StatusCode::UNAUTHORIZED,
                            Json(json!({"success":false,"message":format!("Auth error: {}",e)})),
                        );
                    }
                }
            }
            None => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"success":false,"message":"Missing or invalid Authorization header"})),
                );
            }
        }
    }};
}

// ---------------------------------------------------------------------------
// Query Parameter Structs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListPropertiesParams {
    pub property_type: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub sort_by: Option<String>, // "price_asc" | "price_desc" | "latest" | "popular"
}

#[derive(Debug, Deserialize)]
pub struct BrokerPropertiesParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub status: Option<String>, // "active" | "inactive" | "deleted"
}

#[derive(Debug, Deserialize)]
pub struct PropertySearchParams {
    pub query: Option<String>,
    pub property_type: Option<String>,
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
    pub bedrooms: Option<i32>,
    pub location: Option<String>,
    #[allow(dead_code)]
    pub latitude: Option<f64>,
    #[allow(dead_code)]
    pub longitude: Option<f64>,
    #[allow(dead_code)]
    pub radius_km: Option<f64>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SavedPropertiesParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ---------------------------------------------------------------------------
// Core SELECT fragment (always JOINs users for owner object)
// The caller_user_id param is used to compute is_saved.
// Pass Uuid::nil() for unauthenticated callers.
// ---------------------------------------------------------------------------

/// Builds the property JSON object from a PgRow.
/// `caller_id`: the logged-in user's UUID (Uuid::nil() if not authenticated).
fn row_to_property_json(row: &sqlx::postgres::PgRow, _caller_id: Uuid) -> Value {
    // Parse JSON columns
    let images_val: Value = row.try_get("images").unwrap_or(json!([]));
    let raw_images: Vec<String> = serde_json::from_value(images_val).unwrap_or_default();
    let images: Vec<String> = raw_images
        .into_iter()
        .filter_map(|img| {
            let clean = if let Some(idx) = img.find(" | ") {
                img[..idx].to_string()
            } else {
                img
            };
            if !clean.starts_with("https://") {
                None
            } else {
                Some(clean)
            }
        })
        .collect();

    let amenities_val: Value = row.try_get("amenities").unwrap_or(json!([]));
    let amenities: Vec<String> = serde_json::from_value(amenities_val).unwrap_or_default();

    let nearby_places: Value = row.try_get("nearby_places").unwrap_or(json!({}));

    // Owner object from user join
    let first_name: String = row.try_get("first_name").unwrap_or_default();
    let last_name: String = row.try_get("last_name").unwrap_or_default();
    let owner_name = format!("{} {}", first_name, last_name).trim().to_string();
    let owner_id: Uuid = row.try_get("user_id").unwrap_or(Uuid::nil());
    let owner_phone: Option<String> = row.try_get("phone_no").ok().flatten();
    let owner_profile_image: Option<String> = row.try_get("profile_image").ok().flatten();

    // is_saved and is_liked from query
    let is_saved: bool = row.try_get::<bool, _>("is_saved").unwrap_or(false);
    let is_liked: bool = row.try_get::<bool, _>("is_liked").unwrap_or(false);

    json!({
        "id": row.try_get::<Uuid, _>("id").map(|u| u.to_string()).unwrap_or_default(),
        "title": row.try_get::<Option<String>, _>("title").ok().flatten(),
        "description": row.try_get::<Option<String>, _>("description").ok().flatten(),
        "property_type": row.try_get::<Option<String>, _>("property_type").ok().flatten(),
        "listing_type": row.try_get::<Option<String>, _>("listing_type").ok().flatten(),
        "price": row.try_get::<Option<i64>, _>("price").ok().flatten(),
        "deposit": row.try_get::<Option<i64>, _>("deposit").ok().flatten(),
        "location": row.try_get::<Option<String>, _>("location").ok().flatten(),
        "area_sqft": row.try_get::<Option<i32>, _>("area_sqft").ok().flatten(),
        "bedrooms": row.try_get::<Option<i32>, _>("bedrooms").ok().flatten(),
        "bathrooms": row.try_get::<Option<i32>, _>("bathrooms").ok().flatten(),
        "no_of_toilets": row.try_get::<Option<i32>, _>("no_of_toilets").ok().flatten().unwrap_or(0),
        "no_of_balconies": row.try_get::<Option<i32>, _>("no_of_balconies").ok().flatten().unwrap_or(0),
        "furnishing": row.try_get::<Option<String>, _>("furnishing").ok().flatten(),
        "floor": row.try_get::<Option<i32>, _>("floor").ok().flatten(),
        "total_floors": row.try_get::<Option<i32>, _>("total_floors").ok().flatten(),
        "age_years": row.try_get::<Option<i32>, _>("age_years").ok().flatten(),
        "facing": row.try_get::<Option<String>, _>("facing").ok().flatten(),
        "parking": row.try_get::<Option<bool>, _>("parking").ok().flatten().unwrap_or(false),
        "parking_count": row.try_get::<Option<i32>, _>("parking_count").ok().flatten(),
        "images": images,
        "video_url": row.try_get::<Option<String>, _>("video_url").ok().flatten(),
        "amenities": amenities,
        "nearby_places": nearby_places,
        "latitude": row.try_get::<Option<f64>, _>("latitude").ok().flatten(),
        "longitude": row.try_get::<Option<f64>, _>("longitude").ok().flatten(),
        "is_featured": row.try_get::<Option<bool>, _>("is_featured").ok().flatten().unwrap_or(false),
        "is_verified": row.try_get::<Option<bool>, _>("is_verified").ok().flatten().unwrap_or(false),
        "is_saved": is_saved,
        "is_liked": is_liked,
        "views_count": row.try_get::<Option<i32>, _>("views_count").ok().flatten().unwrap_or(0),
        "likes_count": row.try_get::<Option<i32>, _>("likes_count").ok().flatten().unwrap_or(0),
        "status": row.try_get::<Option<String>, _>("status").ok().flatten(),
        "user_type": row.try_get::<Option<String>, _>("user_type").ok().flatten(),
        "broker_contact_allowed": row.try_get::<Option<bool>, _>("broker_contact_allowed").ok().flatten().unwrap_or(true),
        "created_at": row.try_get::<Option<chrono::DateTime<Utc>>, _>("created_at")
            .ok().flatten().map(|d| d.to_rfc3339()),
        "updated_at": row.try_get::<Option<chrono::DateTime<Utc>>, _>("updated_at")
            .ok().flatten().map(|d| d.to_rfc3339()),
        "owner": {
            "id": owner_id.to_string(),
            "name": owner_name,
            "phone": owner_phone,
            "profile_image": owner_profile_image,
        }
    })
}

/// Returns the base SELECT fragment with user JOIN and is_saved subquery.
/// `caller_id_sql` is the SQL placeholder that represents the current user UUID
/// for computing is_saved. Pass `'00000000-0000-0000-0000-000000000000'::uuid` for anon.
fn property_select_sql(is_saved_bind_pos: usize) -> String {
    format!(
        r#"
        SELECT
            p.id, p.title, p.description, p.property_type, p.listing_type, p.price,
            p.city AS location, p.locality, p.area_sqft, p.bhk AS bedrooms, p.bathrooms,
            p.no_of_toilets, p.no_of_balconies, p.furnishing,
            p.images, p.primary_image, p.amenities,
            p.lat AS latitude, p.lng AS longitude,
            p.deposit, p.floor, p.total_floors, p.age_years, p.facing, p.parking, p.parking_count, p.video_url, p.user_type, p.broker_contact_allowed,
            p.is_featured, p.is_verified,
            p.views_count, p.likes_count,
            p.status, p.user_id, p.created_at, p.updated_at,
            u.first_name, u.last_name, u.phone_no, u.profile_image_url AS profile_image,
            EXISTS(
                SELECT 1 FROM saved_properties sl2
                WHERE sl2.property_id = p.id AND sl2.user_id = ${is_saved_bind_pos}
            ) AS is_saved,
            EXISTS(
                SELECT 1 FROM property_likes pl
                WHERE pl.property_id = p.id AND pl.user_id = ${is_saved_bind_pos}
            ) AS is_liked
        FROM properties p
        LEFT JOIN users u ON p.user_id = u.id
        "#,
        is_saved_bind_pos = is_saved_bind_pos
    )
}

// ---------------------------------------------------------------------------
// 1. GET /api/properties
// ---------------------------------------------------------------------------

pub async fn get_properties(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ListPropertiesParams>,
) -> impl axum::response::IntoResponse {
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    let order_by = match params.sort_by.as_deref() {
        Some("price_asc") => "p.price ASC",
        Some("price_desc") => "p.price DESC",
        Some("popular") => "p.likes_count DESC",
        _ => "p.created_at DESC",
    };

    // Determine caller for is_saved and is_liked (optional auth)
    let caller = {
        let bearer = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer ").map(|s| s.to_string()));
        if let Some(b) = bearer {
            let dk = DecodingKey::from_secret(app_state.jwt_secret.as_bytes());
            extract_user_id_from_jwt(&b, &dk).unwrap_or(Uuid::nil())
        } else {
            Uuid::nil()
        }
    };

    // is_saved bind is $1 (caller), then property_type optional, then limit/offset
    let mut conditions = vec!["p.status = 'active'".to_string()];
    let mut extra_binds: Vec<String> = vec![];
    let mut bind_offset = 2usize; // $1 = caller

    if let Some(ref pt) = params.property_type {
        conditions.push(format!("p.property_type = ${}", bind_offset));
        extra_binds.push(pt.clone());
        bind_offset += 1;
    }

    let where_clause = conditions.join(" AND ");
    let limit_pos = bind_offset;
    let offset_pos = bind_offset + 1;

    let sql = format!(
        "{} WHERE {} ORDER BY {} LIMIT ${} OFFSET ${}",
        property_select_sql(1),
        where_clause,
        order_by,
        limit_pos,
        offset_pos
    );

    let mut q = sqlx::query(&sql).bind(caller);
    for b in &extra_binds {
        q = q.bind(b.clone());
    }
    q = q.bind(limit).bind(offset);

    let rows = match q.fetch_all(&app_state.db).await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success":false,"message":format!("Database error: {}",e)})),
            );
        }
    };

    // Count query
    let count_sql = if let Some(ref pt) = params.property_type {
        format!(
            "SELECT COUNT(*) FROM properties p WHERE p.status = 'active' AND p.property_type = '{}'",
            pt.replace('\'', "''")
        )
    } else {
        "SELECT COUNT(*) FROM properties p WHERE p.status = 'active'".to_string()
    };
    let total: i64 = sqlx::query_scalar(&count_sql)
        .fetch_one(&app_state.db)
        .await
        .unwrap_or(0);

    let properties: Vec<Value> = rows
        .iter()
        .map(|r| row_to_property_json(r, caller))
        .collect();

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Properties fetched successfully",
            "data": {
                "properties": properties,
                "pagination": {
                    "total": total,
                    "limit": limit,
                    "offset": offset,
                    "has_more": offset + limit < total
                }
            }
        })),
    )
}

// ---------------------------------------------------------------------------
// 2. GET /api/properties/{property_id}
// ---------------------------------------------------------------------------

pub async fn get_property_by_id(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl axum::response::IntoResponse {
    let property_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success":false,"message":"Invalid property id"})),
            );
        }
    };

    // Determine caller for is_saved (optional auth)
    let caller = {
        let bearer = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer ").map(|s| s.to_string()));
        if let Some(b) = bearer {
            let dk = DecodingKey::from_secret(app_state.jwt_secret.as_bytes());
            extract_user_id_from_jwt(&b, &dk).unwrap_or(Uuid::nil())
        } else {
            Uuid::nil()
        }
    };

    // Increment views_count
    let _ = sqlx::query("UPDATE properties SET views_count = views_count + 1 WHERE id = $1")
        .bind(property_id)
        .execute(&app_state.db)
        .await;

    // $1 = caller (for is_saved), $2 = property_id
    let sql = format!(
        "{} WHERE p.id = $2 AND p.status != 'deleted'",
        property_select_sql(1)
    );

    let row = match sqlx::query(&sql)
        .bind(caller)
        .bind(property_id)
        .fetch_optional(&app_state.db)
        .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"success":false,"message":"Property not found"})),
            );
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success":false,"message":format!("Database error: {}",e)})),
            );
        }
    };

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Property fetched successfully",
            "data": { "property": row_to_property_json(&row, caller) }
        })),
    )
}

// ---------------------------------------------------------------------------
// 3. POST /api/properties  — create property
// ---------------------------------------------------------------------------

pub async fn create_property(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreatePropertyRequest>,
) -> impl axum::response::IntoResponse {
    let user_id = require_auth!(headers, app_state);

    let property_id = Uuid::new_v4();
    let now = Utc::now();

    fn clean_image_url(url: &str) -> String {
        let clean = if let Some(idx) = url.find(" | ") {
            url[..idx].to_string()
        } else {
            url.to_string()
        };
        clean
    }

    let raw_images = payload.images.unwrap_or_default();
    let cleaned_images: Vec<String> = raw_images
        .into_iter()
        .map(|img| clean_image_url(&img))
        .filter(|img| img.starts_with("https://"))
        .collect();
    let images_json = serde_json::to_value(cleaned_images).unwrap_or(json!([]));

    let amenities_json =
        serde_json::to_value(payload.amenities.unwrap_or_default()).unwrap_or(json!([]));
    let _nearby_json = payload.nearby_places.unwrap_or(json!({}));

    let result = sqlx::query(
        r#"
        INSERT INTO properties (
            id, title, description, property_type, listing_type, price,
            city, area_sqft, bhk, bathrooms, no_of_toilets, no_of_balconies, furnishing,
            images, amenities, lat, lng,
            deposit, floor, total_floors, age_years, facing, parking, parking_count, video_url, user_type, broker_contact_allowed,
            is_featured, is_verified,
            status, user_id, created_at, updated_at
        ) VALUES (
            $1, $2, $3, $4, $5, $6,
            $7, $8, $9, $10, $11, $12, $13,
            $14, $15, $16, $17,
            $18, $19, $20, $21, $22, $23, $24, $25, $26, $27,
            false, false,
            'active', $28, $29, $29
        )
        RETURNING id
        "#,
    )
    .bind(property_id)
    .bind(&payload.title)
    .bind(&payload.description)
    .bind(&payload.property_type)
    .bind(payload.listing_type.as_deref().unwrap_or("Rent"))
    .bind(payload.price)
    .bind(&payload.location)   // city
    .bind(payload.area_sqft)
    .bind(payload.bedrooms)    // bhk
    .bind(payload.bathrooms)
    .bind(payload.no_of_toilets.unwrap_or(0))
    .bind(payload.no_of_balconies.unwrap_or(0))
    .bind(&payload.furnishing)
    .bind(images_json)
    .bind(amenities_json)
    .bind(payload.latitude)    // lat
    .bind(payload.longitude)   // lng
    .bind(payload.deposit)
    .bind(payload.floor)
    .bind(payload.total_floors)
    .bind(payload.age_years)
    .bind(&payload.facing)
    .bind(payload.parking)
    .bind(payload.parking_count)
    .bind(&payload.video_url)
    .bind(&payload.user_type)
    .bind(payload.broker_contact_allowed)
    .bind(user_id)
    .bind(now)
    .fetch_one(&app_state.db)
    .await;

    match result {
        Ok(row) => {
            let id: Uuid = row.get("id");
            (
                StatusCode::CREATED,
                Json(json!({
                    "success": true,
                    "message": "Property created successfully",
                    "data": { "property_id": id.to_string() }
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success":false,"message":format!("Database error: {}",e)})),
        ),
    }
}

// ---------------------------------------------------------------------------
// 4. PUT /api/properties/{property_id}
// ---------------------------------------------------------------------------

pub async fn update_property(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<UpdatePropertyRequest>,
) -> impl axum::response::IntoResponse {
    let user_id = require_auth!(headers, app_state);

    let property_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success":false,"message":"Invalid property id"})),
            );
        }
    };

    // Ownership check
    let owner: Option<Uuid> = match sqlx::query_scalar(
        "SELECT user_id FROM properties WHERE id = $1 AND status != 'deleted'",
    )
    .bind(property_id)
    .fetch_optional(&app_state.db)
    .await
    {
        Ok(o) => o,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success":false,"message":format!("Database error: {}",e)})),
            );
        }
    };

    match owner {
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"success":false,"message":"Property not found"})),
            );
        }
        Some(o) if o != user_id => {
            return (
                StatusCode::FORBIDDEN,
                Json(
                    json!({"success":false,"message":"You don't have permission to update this property"}),
                ),
            );
        }
        _ => {}
    }

    let updated_at = Utc::now();
    let mut qb: sqlx::QueryBuilder<sqlx::Postgres> =
        sqlx::QueryBuilder::new("UPDATE properties SET updated_at = ");
    qb.push_bind(updated_at);

    if let Some(v) = &payload.title {
        qb.push(", title = ");
        qb.push_bind(v);
    }
    if let Some(v) = &payload.description {
        qb.push(", description = ");
        qb.push_bind(v);
    }
    if let Some(v) = &payload.property_type {
        qb.push(", property_type = ");
        qb.push_bind(v);
    }
    if let Some(v) = payload.price {
        qb.push(", price = ");
        qb.push_bind(v);
    }
    if let Some(v) = &payload.location {
        qb.push(", city = "); // location maps to city
        qb.push_bind(v);
    }
    if let Some(v) = payload.area_sqft {
        qb.push(", area_sqft = ");
        qb.push_bind(v);
    }
    if let Some(v) = payload.bedrooms {
        qb.push(", bhk = "); // bedrooms maps to bhk
        qb.push_bind(v);
    }
    if let Some(v) = payload.bathrooms {
        qb.push(", bathrooms = ");
        qb.push_bind(v);
    }
    if let Some(v) = payload.no_of_toilets {
        qb.push(", no_of_toilets = ");
        qb.push_bind(v);
    }
    if let Some(v) = payload.no_of_balconies {
        qb.push(", no_of_balconies = ");
        qb.push_bind(v);
    }
    if let Some(v) = &payload.furnishing {
        qb.push(", furnishing = ");
        qb.push_bind(v);
    }
    if let Some(v) = payload.latitude {
        qb.push(", lat = "); // latitude maps to lat
        qb.push_bind(v);
    }
    if let Some(v) = payload.longitude {
        qb.push(", lng = "); // longitude maps to lng
        qb.push_bind(v);
    }
    if let Some(v) = &payload.status {
        qb.push(", status = ");
        qb.push_bind(v);
    }
    if let Some(v) = &payload.images {
        let cleaned_images: Vec<String> = v
            .iter()
            .filter_map(|img| {
                let clean = if let Some(idx) = img.find(" | ") {
                    img[..idx].to_string()
                } else {
                    img.to_string()
                };
                if !clean.starts_with("https://") {
                    None
                } else {
                    Some(clean)
                }
            })
            .collect();
        let j = serde_json::to_value(cleaned_images).unwrap_or(json!([]));
        qb.push(", images = ");
        qb.push_bind(j);
    }
    if let Some(v) = &payload.amenities {
        let j = serde_json::to_value(v).unwrap_or(json!([]));
        qb.push(", amenities = ");
        qb.push_bind(j);
    }
    if let Some(v) = payload.deposit {
        qb.push(", deposit = ");
        qb.push_bind(v);
    }
    if let Some(v) = payload.floor {
        qb.push(", floor = ");
        qb.push_bind(v);
    }
    if let Some(v) = payload.total_floors {
        qb.push(", total_floors = ");
        qb.push_bind(v);
    }
    if let Some(v) = payload.age_years {
        qb.push(", age_years = ");
        qb.push_bind(v);
    }
    if let Some(v) = &payload.facing {
        qb.push(", facing = ");
        qb.push_bind(v);
    }
    if let Some(v) = payload.parking {
        qb.push(", parking = ");
        qb.push_bind(v);
    }
    if let Some(v) = payload.parking_count {
        qb.push(", parking_count = ");
        qb.push_bind(v);
    }
    if let Some(v) = &payload.video_url {
        qb.push(", video_url = ");
        qb.push_bind(v);
    }
    if let Some(v) = &payload.user_type {
        qb.push(", user_type = ");
        qb.push_bind(v);
    }
    if let Some(v) = payload.broker_contact_allowed {
        qb.push(", broker_contact_allowed = ");
        qb.push_bind(v);
    }

    qb.push(" WHERE id = ");
    qb.push_bind(property_id);

    if let Err(e) = qb.build().execute(&app_state.db).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success":false,"message":format!("Update failed: {}",e)})),
        );
    }

    // Fetch updated property — $1 = caller (owner), $2 = property_id
    let sql = format!("{} WHERE p.id = $2", property_select_sql(1));
    let row = match sqlx::query(&sql)
        .bind(user_id)
        .bind(property_id)
        .fetch_one(&app_state.db)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success":false,"message":format!("Fetch error: {}",e)})),
            );
        }
    };

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Property updated successfully",
            "data": { "property": row_to_property_json(&row, user_id) }
        })),
    )
}

// ---------------------------------------------------------------------------
// 5. DELETE /api/properties/{property_id}  — soft delete
// ---------------------------------------------------------------------------

pub async fn delete_property(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl axum::response::IntoResponse {
    let user_id = require_auth!(headers, app_state);

    let property_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success":false,"message":"Invalid property id"})),
            );
        }
    };

    let owner: Option<Uuid> = match sqlx::query_scalar(
        "SELECT user_id FROM properties WHERE id = $1 AND status != 'deleted'",
    )
    .bind(property_id)
    .fetch_optional(&app_state.db)
    .await
    {
        Ok(o) => o,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success":false,"message":format!("Database error: {}",e)})),
            );
        }
    };

    match owner {
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"success":false,"message":"Property not found"})),
            );
        }
        Some(o) if o != user_id => {
            return (
                StatusCode::FORBIDDEN,
                Json(
                    json!({"success":false,"message":"You don't have permission to delete this property"}),
                ),
            );
        }
        _ => {}
    }

    let _ = sqlx::query("UPDATE properties SET status = 'deleted', updated_at = $1 WHERE id = $2")
        .bind(Utc::now())
        .bind(property_id)
        .execute(&app_state.db)
        .await;

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Property deleted successfully",
            "data": { "property_id": property_id.to_string() }
        })),
    )
}

// ---------------------------------------------------------------------------
// 6. GET /api/properties/broker  — my (owner's) properties
// ---------------------------------------------------------------------------

pub async fn get_broker_properties(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<BrokerPropertiesParams>,
) -> impl axum::response::IntoResponse {
    let user_id = require_auth!(headers, app_state);
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    // status filter
    let status_filter = params.status.as_deref().unwrap_or("active");

    // $1 = caller (is_saved), $2 = user_id (owner), $3 = status, $4 = limit, $5 = offset
    let sql = format!(
        "{} WHERE p.user_id = $2 AND p.status = $3 ORDER BY p.created_at DESC LIMIT $4 OFFSET $5",
        property_select_sql(1)
    );

    let rows = match sqlx::query(&sql)
        .bind(user_id)
        .bind(user_id)
        .bind(status_filter)
        .bind(limit)
        .bind(offset)
        .fetch_all(&app_state.db)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success":false,"message":format!("Database error: {}",e)})),
            );
        }
    };

    let total: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM properties WHERE user_id = $1 AND status = $2")
            .bind(user_id)
            .bind(status_filter)
            .fetch_one(&app_state.db)
            .await
            .unwrap_or(0);

    let properties: Vec<Value> = rows
        .iter()
        .map(|r| row_to_property_json(r, user_id))
        .collect();

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Your properties fetched successfully",
            "data": {
                "properties": properties,
                "pagination": { "total": total, "limit": limit, "offset": offset }
            }
        })),
    )
}

// ---------------------------------------------------------------------------
// 7. GET /api/properties/search
// ---------------------------------------------------------------------------

pub async fn search_properties(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<PropertySearchParams>,
) -> impl axum::response::IntoResponse {
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    let caller = {
        let bearer = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer ").map(|s| s.to_string()));
        if let Some(b) = bearer {
            let dk = DecodingKey::from_secret(app_state.jwt_secret.as_bytes());
            extract_user_id_from_jwt(&b, &dk).unwrap_or(Uuid::nil())
        } else {
            Uuid::nil()
        }
    };

    // $1 = caller for is_saved; extra conditions start at $2
    let mut conditions = vec!["p.status = 'active'".to_string()];
    let mut next_bind = 2usize;

    // We'll store typed enum to avoid type erasure issues
    enum BindVal {
        Str(String),
        I64(i64),
        I32(i32),
    }
    let mut binds: Vec<BindVal> = vec![];

    if let Some(ref q) = params.query {
        conditions.push(format!(
            "(p.title ILIKE ${b} OR p.description ILIKE ${b} OR p.location ILIKE ${b})",
            b = next_bind
        ));
        binds.push(BindVal::Str(format!("%{}%", q)));
        next_bind += 1;
    }
    if let Some(ref pt) = params.property_type {
        conditions.push(format!("p.property_type = ${}", next_bind));
        binds.push(BindVal::Str(pt.clone()));
        next_bind += 1;
    }
    if let Some(v) = params.min_price {
        conditions.push(format!("p.price >= ${}", next_bind));
        binds.push(BindVal::I64(v));
        next_bind += 1;
    }
    if let Some(v) = params.max_price {
        conditions.push(format!("p.price <= ${}", next_bind));
        binds.push(BindVal::I64(v));
        next_bind += 1;
    }
    if let Some(v) = params.bedrooms {
        conditions.push(format!("p.bedrooms = ${}", next_bind));
        binds.push(BindVal::I32(v));
        next_bind += 1;
    }
    if let Some(ref loc) = params.location {
        conditions.push(format!("p.location ILIKE ${}", next_bind));
        binds.push(BindVal::Str(format!("%{}%", loc)));
        next_bind += 1;
    }

    let where_clause = conditions.join(" AND ");
    let sql = format!(
        "{} WHERE {} ORDER BY p.created_at DESC LIMIT ${} OFFSET ${}",
        property_select_sql(1),
        where_clause,
        next_bind,
        next_bind + 1
    );

    let mut q = sqlx::query(&sql).bind(caller);
    for b in &binds {
        q = match b {
            BindVal::Str(s) => q.bind(s.clone()),
            BindVal::I64(n) => q.bind(*n),
            BindVal::I32(n) => q.bind(*n),
        };
    }
    q = q.bind(limit).bind(offset);

    let rows = match q.fetch_all(&app_state.db).await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success":false,"message":format!("Database error: {}",e)})),
            );
        }
    };

    let properties: Vec<Value> = rows
        .iter()
        .map(|r| row_to_property_json(r, caller))
        .collect();

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Search results",
            "data": {
                "properties": properties,
                "count": properties.len(),
                "pagination": { "limit": limit, "offset": offset }
            }
        })),
    )
}

// ---------------------------------------------------------------------------
// 8. POST /api/properties/{property_id}/like
// ---------------------------------------------------------------------------

pub async fn like_property(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl axum::response::IntoResponse {
    let user_id = require_auth!(headers, app_state);

    let property_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success":false,"message":"Invalid property id"})),
            );
        }
    };

    let already: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM property_likes WHERE property_id = $1 AND user_id = $2")
            .bind(property_id)
            .bind(user_id)
            .fetch_optional(&app_state.db)
            .await
            .unwrap_or(None);

    if already.is_none() {
        let _ = sqlx::query(
            "INSERT INTO property_likes (id, property_id, user_id, created_at) VALUES ($1,$2,$3,$4)",
        )
        .bind(Uuid::new_v4())
        .bind(property_id)
        .bind(user_id)
        .bind(Utc::now())
        .execute(&app_state.db)
        .await;

        let _ = sqlx::query("UPDATE properties SET likes_count = likes_count + 1 WHERE id = $1")
            .bind(property_id)
            .execute(&app_state.db)
            .await;
    }

    let likes_count: i32 = sqlx::query_scalar("SELECT likes_count FROM properties WHERE id = $1")
        .bind(property_id)
        .fetch_one(&app_state.db)
        .await
        .unwrap_or(0);

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": if already.is_none() { "Property liked" } else { "Already liked" },
            "data": {
                "property_id": property_id.to_string(),
                "liked": true,
                "likes_count": likes_count
            }
        })),
    )
}

// ---------------------------------------------------------------------------
// 9. DELETE /api/properties/{property_id}/like
// ---------------------------------------------------------------------------

pub async fn unlike_property(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl axum::response::IntoResponse {
    let user_id = require_auth!(headers, app_state);

    let property_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success":false,"message":"Invalid property id"})),
            );
        }
    };

    let result = sqlx::query("DELETE FROM property_likes WHERE property_id = $1 AND user_id = $2")
        .bind(property_id)
        .bind(user_id)
        .execute(&app_state.db)
        .await;

    if let Ok(r) = result {
        if r.rows_affected() > 0 {
            let _ = sqlx::query(
                "UPDATE properties SET likes_count = GREATEST(likes_count - 1, 0) WHERE id = $1",
            )
            .bind(property_id)
            .execute(&app_state.db)
            .await;
        }
    }

    let likes_count: i32 = sqlx::query_scalar("SELECT likes_count FROM properties WHERE id = $1")
        .bind(property_id)
        .fetch_one(&app_state.db)
        .await
        .unwrap_or(0);

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Property unliked",
            "data": {
                "property_id": property_id.to_string(),
                "liked": false,
                "likes_count": likes_count
            }
        })),
    )
}

// ---------------------------------------------------------------------------
// 10. POST /api/properties/{property_id}/save
// ---------------------------------------------------------------------------

pub async fn save_property(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl axum::response::IntoResponse {
    let user_id = require_auth!(headers, app_state);

    let property_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success":false,"message":"Invalid property id"})),
            );
        }
    };

    let already: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM saved_properties WHERE property_id = $1 AND user_id = $2",
    )
    .bind(property_id)
    .bind(user_id)
    .fetch_optional(&app_state.db)
    .await
    .unwrap_or(None);

    if already.is_none() {
        let _ = sqlx::query(
            "INSERT INTO saved_properties (id, property_id, user_id, created_at) VALUES ($1,$2,$3,$4)",
        )
        .bind(Uuid::new_v4())
        .bind(property_id)
        .bind(user_id)
        .bind(Utc::now())
        .execute(&app_state.db)
        .await;
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": if already.is_none() { "Property saved" } else { "Already saved" },
            "data": { "property_id": property_id.to_string(), "is_saved": true }
        })),
    )
}

// ---------------------------------------------------------------------------
// 11. DELETE /api/properties/{property_id}/save
// ---------------------------------------------------------------------------

pub async fn unsave_property(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl axum::response::IntoResponse {
    let user_id = require_auth!(headers, app_state);

    let property_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success":false,"message":"Invalid property id"})),
            );
        }
    };

    let _ = sqlx::query("DELETE FROM saved_properties WHERE property_id = $1 AND user_id = $2")
        .bind(property_id)
        .bind(user_id)
        .execute(&app_state.db)
        .await;

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Property removed from saved",
            "data": { "property_id": property_id.to_string(), "is_saved": false }
        })),
    )
}

// ---------------------------------------------------------------------------
// 12. POST /api/properties/{property_id}/report
// ---------------------------------------------------------------------------

pub async fn report_property(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<ReportPropertyRequest>,
) -> impl axum::response::IntoResponse {
    let user_id = require_auth!(headers, app_state);

    let property_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success":false,"message":"Invalid property id"})),
            );
        }
    };

    let exists: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM properties WHERE id = $1 AND status != 'deleted'")
            .bind(property_id)
            .fetch_optional(&app_state.db)
            .await
            .unwrap_or(None);

    if exists.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"success":false,"message":"Property not found"})),
        );
    }

    let result = sqlx::query(
        r#"
        INSERT INTO property_reports (id, property_id, reporter_id, reason, description, created_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(property_id)
    .bind(user_id)
    .bind(&payload.reason)
    .bind(&payload.description)
    .bind(Utc::now())
    .execute(&app_state.db)
    .await;

    match result {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Property reported successfully",
                "data": { "property_id": property_id.to_string() }
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success":false,"message":format!("Failed to report: {}",e)})),
        ),
    }
}

// ---------------------------------------------------------------------------
// 13. GET /api/properties/saved
// ---------------------------------------------------------------------------

pub async fn get_saved_properties(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<SavedPropertiesParams>,
) -> impl axum::response::IntoResponse {
    let user_id = require_auth!(headers, app_state);
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    // $1 = caller (is_saved, always true here), $2 = limit, $3 = offset
    let sql = r#"
        SELECT
            p.id, p.title, p.description, p.property_type, p.price,
            p.city AS location, p.locality, p.area_sqft, p.bhk AS bedrooms, p.bathrooms,
            p.no_of_toilets, p.no_of_balconies, p.furnishing,
            p.images, p.primary_image, p.amenities,
            p.lat AS latitude, p.lng AS longitude,
            p.is_featured, p.is_verified, p.views_count, p.likes_count,
            p.status, p.user_id, p.created_at, p.updated_at,
            u.first_name, u.last_name, u.phone_no, u.profile_image_url AS profile_image,
            true AS is_saved,
            EXISTS(
                SELECT 1 FROM property_likes pl
                WHERE pl.property_id = p.id AND pl.user_id = $1
            ) AS is_liked
        FROM saved_properties sp
        JOIN properties p ON sp.property_id = p.id
        LEFT JOIN users u ON p.user_id = u.id
        WHERE sp.user_id = $1 AND p.status != 'deleted'
        ORDER BY sp.created_at DESC
        LIMIT $2 OFFSET $3
    "#;

    let rows = match sqlx::query(sql)
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&app_state.db)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success":false,"message":format!("Database error: {}",e)})),
            );
        }
    };

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM saved_properties sp JOIN properties p ON sp.property_id = p.id WHERE sp.user_id = $1 AND p.status != 'deleted'"
    )
    .bind(user_id)
    .fetch_one(&app_state.db)
    .await
    .unwrap_or(0);

    let properties: Vec<Value> = rows
        .iter()
        .map(|r| row_to_property_json(r, user_id))
        .collect();

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Saved properties fetched successfully",
            "data": {
                "properties": properties,
                "pagination": { "total": total, "limit": limit, "offset": offset }
            }
        })),
    )
}
