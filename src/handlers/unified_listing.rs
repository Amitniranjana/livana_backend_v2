use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use axum_extra::extract::Multipart;

use crate::{
    app_state::AppState,
    dtos::unified_listing::{
        CreateListingPayload, ListingDetail, ListingFilters, ListingImageRow, ListingSummary, ListingDraftPayload, ListingDraftResponse
    },
    handlers::listing_validator::{auto_derive_parking, validate_listing},
    services::storage::StorageService,
    utils::auth_extractor::AuthenticationUser,
};

// ─────────────────────────────────────────────────────────────────────────────
// 1. POST /api/listings — Create Listing
// ─────────────────────────────────────────────────────────────────────────────

pub async fn create_listing(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Json(payload): Json<CreateListingPayload>,
) -> impl IntoResponse {
    let user_id = match Uuid::parse_str(&auth.user_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"success": false, "message": "Invalid user ID in token"})),
            );
        }
    };

    // ── Validate payload ────────────────────────────────────────────────────
    if let Err(errors) = validate_listing(&payload) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "message": "Validation failed",
                "errors": errors
            })),
        );
    }

    // ── Auto-derive parking ─────────────────────────────────────────────────
    let parking = auto_derive_parking(&payload);

    let listing_id = Uuid::new_v4();
    let now = Utc::now();

    let amenities: Vec<String> = payload.amenities.unwrap_or_default();

    // ── Insert into listings_v2 ─────────────────────────────────────────────
    let result = sqlx::query(
        r#"
        INSERT INTO listings_v2 (
            id, title, description,
            property_type, listing_type, user_type, host,
            price, deposit,
            location, area, city, pincode,
            latitude, longitude,
            area_sqft,
            bedrooms, bathrooms, no_of_toilets, no_of_balconies,
            furnishing, facing,
            floor, total_floors,
            commercial_type, land_type,
            lease_years, bathroom_type,
            gender_preference, roommates,
            amenities, parking, broker_contact_allowed,
            age_years,
            created_by, created_at, updated_at
        ) VALUES (
            $1, $2, $3,
            $4, $5, $6, $7,
            $8, $9,
            $10, $11, $12, $13,
            $14, $15,
            $16,
            $17, $18, $19, $20,
            $21, $22,
            $23, $24,
            $25, $26,
            $27, $28,
            $29, $30,
            $31, $32, $33,
            $34,
            $35, $36, $36
        )
        RETURNING id
        "#,
    )
    .bind(listing_id)                          // $1
    .bind(&payload.title)                      // $2
    .bind(&payload.description)                // $3
    .bind(&payload.property_type)              // $4
    .bind(&payload.listing_type)               // $5
    .bind(&payload.user_type)                  // $6
    .bind(&payload.host)                       // $7
    .bind(payload.price)                       // $8
    .bind(payload.deposit)                     // $9
    .bind(&payload.location)                   // $10
    .bind(&payload.area)                       // $11
    .bind(&payload.city)                       // $12
    .bind(&payload.pincode)                    // $13
    .bind(payload.latitude)                    // $14
    .bind(payload.longitude)                   // $15
    .bind(payload.area_sqft)                   // $16
    .bind(payload.bedrooms)                    // $17
    .bind(payload.bathrooms)                   // $18
    .bind(payload.no_of_toilets)               // $19
    .bind(payload.no_of_balconies)             // $20
    .bind(&payload.furnishing)                 // $21
    .bind(&payload.facing)                     // $22
    .bind(payload.floor)                       // $23
    .bind(payload.total_floors)                // $24
    .bind(&payload.commercial_type)            // $25
    .bind(&payload.land_type)                  // $26
    .bind(payload.lease_years)                 // $27
    .bind(&payload.bathroom_type)              // $28
    .bind(&payload.gender_preference)          // $29
    .bind(payload.roommates)                   // $30
    .bind(&amenities)                          // $31
    .bind(parking)                             // $32
    .bind(payload.broker_contact_allowed.unwrap_or(true))  // $33
    .bind(payload.age_years)                   // $34
    .bind(user_id)                             // $35  created_by from JWT
    .bind(now)                                 // $36  created_at & updated_at
    .fetch_one(&app_state.db)
    .await;

    match result {
        Ok(row) => {
            let id: Uuid = row.get("id");

            // ── If images provided, insert them into listing_images_v2 ──
            if let Some(ref urls) = payload.images {
                for (i, url) in urls.iter().enumerate() {
                    if url.trim().is_empty() || !url.starts_with("https://") {
                        continue;
                    }
                    let _ = sqlx::query(
                        r#"
                        INSERT INTO listing_images_v2 (id, listing_id, image_url, display_order, created_at)
                        VALUES ($1, $2, $3, $4, $5)
                        "#,
                    )
                    .bind(Uuid::new_v4())
                    .bind(id)
                    .bind(url)
                    .bind((i + 1) as i32)
                    .bind(now)
                    .execute(&app_state.db)
                    .await;
                }
            }

            // ── Area-Based Notifications ──
            let db_clone = app_state.db.clone();
            let listing_title = payload.title.clone();
            let listing_id_str = id.to_string();
            let city_clone = payload.city.clone().unwrap_or_default();
            let location_clone = payload.location.clone();

            tokio::spawn(async move {
                let matching_users: Vec<Uuid> = sqlx::query_scalar(
                    r#"
                    SELECT id FROM users 
                    WHERE selected_area IS NOT NULL 
                    AND (selected_area ILIKE $1 OR selected_area ILIKE $2)
                    "#,
                )
                .bind(format!("%{}%", city_clone))
                .bind(format!("%{}%", location_clone))
                .fetch_all(&db_clone)
                .await
                .unwrap_or_default();

                for uid in matching_users {
                    let _ = crate::utils::notification_chat_helper::create_notification(
                        &db_clone,
                        uid,
                        "New Listing in Your Area",
                        &format!("A new listing '{}' has been added in your area.", listing_title),
                        "SYSTEM",
                        Uuid::parse_str(&listing_id_str).ok(),
                        Some("Listing"),
                    )
                    .await;
                }
            });

            (
                StatusCode::CREATED,
                Json(json!({
                    "success": true,
                    "data": {
                        "property_id": id.to_string()
                    }
                })),
            )
        }
        Err(e) => {
            log::error!("Failed to create listing: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": format!("Database error: {}", e)
                })),
            )
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. GET /api/listings/{id} — Get Listing Details
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_listing_by_id(
    State(app_state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let listing_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "Invalid listing id"})),
            );
        }
    };

    // ── Try Redis cache first ───────────────────────────────────────────────
    if let Some(ref redis_pool) = app_state.redis_pool {
        let cache_key = format!("listing:{}", listing_id);
        let mut conn = redis_pool.clone();
        if let Ok(cached) = redis::AsyncCommands::get::<_, Option<String>>(&mut conn, &cache_key).await {
            if let Some(json_str) = cached {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    log::info!("Cache HIT for listing {}", listing_id);
                    return (
                        StatusCode::OK,
                        Json(json!({
                            "success": true,
                            "data": parsed
                        })),
                    );
                }
            }
        }
    }

    // ── Fetch listing from DB ───────────────────────────────────────────────
    let row = match sqlx::query(
        r#"
        SELECT
            id, title, description,
            property_type, listing_type, user_type, host,
            price, deposit,
            location, area, city, pincode,
            latitude, longitude,
            area_sqft,
            bedrooms, bathrooms, no_of_toilets, no_of_balconies,
            furnishing, facing,
            floor, total_floors,
            commercial_type, land_type,
            lease_years, bathroom_type,
            gender_preference, roommates,
            amenities, parking, broker_contact_allowed,
            age_years,
            created_by, created_at, updated_at
        FROM listings_v2
        WHERE id = $1
        "#,
    )
    .bind(listing_id)
    .fetch_optional(&app_state.db)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"success": false, "message": "Listing not found"})),
            );
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": format!("Database error: {}", e)})),
            );
        }
    };

    // ── Fetch images ────────────────────────────────────────────────────────
    let image_rows = sqlx::query(
        r#"
        SELECT id, image_url, display_order, created_at
        FROM listing_images_v2
        WHERE listing_id = $1
        ORDER BY display_order ASC
        "#,
    )
    .bind(listing_id)
    .fetch_all(&app_state.db)
    .await
    .unwrap_or_default();

    let images: Vec<ListingImageRow> = image_rows
        .iter()
        .map(|r| ListingImageRow {
            id: r
                .try_get::<Uuid, _>("id")
                .map(|u| u.to_string())
                .unwrap_or_default(),
            image_url: r.try_get("image_url").unwrap_or_default(),
            display_order: r.try_get("display_order").ok(),
            created_at: r
                .try_get::<Option<chrono::DateTime<Utc>>, _>("created_at")
                .ok()
                .flatten()
                .map(|d| d.to_rfc3339()),
        })
        .collect();

    let amenities_raw: Vec<String> = row.try_get("amenities").unwrap_or_default();

    let detail = ListingDetail {
        id: row
            .try_get::<Uuid, _>("id")
            .map(|u| u.to_string())
            .unwrap_or_default(),
        title: row.try_get("title").unwrap_or_default(),
        description: row.try_get("description").unwrap_or_default(),
        property_type: row.try_get("property_type").unwrap_or_default(),
        listing_type: row.try_get("listing_type").unwrap_or_default(),
        user_type: row.try_get("user_type").unwrap_or_default(),
        host: row.try_get("host").ok(),
        price: row.try_get("price").unwrap_or_default(),
        deposit: row.try_get("deposit").unwrap_or_default(),
        location: row.try_get("location").unwrap_or_default(),
        area: row.try_get("area").ok(),
        city: row.try_get("city").ok(),
        pincode: row.try_get("pincode").ok(),
        latitude: row.try_get("latitude").ok(),
        longitude: row.try_get("longitude").ok(),
        area_sqft: row.try_get("area_sqft").unwrap_or_default(),
        bedrooms: row.try_get("bedrooms").ok(),
        bathrooms: row.try_get("bathrooms").ok(),
        no_of_toilets: row.try_get("no_of_toilets").ok(),
        no_of_balconies: row.try_get("no_of_balconies").ok(),
        furnishing: row.try_get("furnishing").ok(),
        facing: row.try_get("facing").ok(),
        floor: row.try_get("floor").ok(),
        total_floors: row.try_get("total_floors").ok(),
        commercial_type: row.try_get("commercial_type").ok(),
        land_type: row.try_get("land_type").ok(),
        lease_years: row.try_get("lease_years").ok(),
        bathroom_type: row.try_get("bathroom_type").ok(),
        gender_preference: row.try_get("gender_preference").ok(),
        roommates: row.try_get("roommates").ok(),
        amenities: Some(amenities_raw),
        parking: row.try_get("parking").ok(),
        broker_contact_allowed: row.try_get("broker_contact_allowed").ok(),
        age_years: row.try_get("age_years").ok(),
        created_by: row
            .try_get::<Uuid, _>("created_by")
            .map(|u| u.to_string())
            .unwrap_or_default(),
        created_at: row
            .try_get::<Option<chrono::DateTime<Utc>>, _>("created_at")
            .ok()
            .flatten()
            .map(|d| d.to_rfc3339()),
        updated_at: row
            .try_get::<Option<chrono::DateTime<Utc>>, _>("updated_at")
            .ok()
            .flatten()
            .map(|d| d.to_rfc3339()),
        images,
    };

    // ── Cache in Redis (60s TTL) ────────────────────────────────────────────
    if let Some(ref redis_pool) = app_state.redis_pool {
        let cache_key = format!("listing:{}", listing_id);
        let mut conn = redis_pool.clone();
        if let Ok(json_str) = serde_json::to_string(&detail) {
            let _: Result<(), _> =
                redis::AsyncCommands::set_ex(&mut conn, &cache_key, &json_str, 60).await;
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": detail
        })),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. GET /api/listings — List with pagination + filtering
// ─────────────────────────────────────────────────────────────────────────────

pub async fn list_listings(
    State(app_state): State<AppState>,
    Query(filters): Query<ListingFilters>,
) -> impl IntoResponse {
    let limit = filters.limit.unwrap_or(20).min(100);
    let offset = filters.offset.unwrap_or(0);

    let order_by = match filters.sort_by.as_deref() {
        Some("price_asc") => "l.price ASC",
        Some("price_desc") => "l.price DESC",
        _ => "l.created_at DESC",
    };

    // Build dynamic WHERE clause
    let mut conditions: Vec<String> = vec![];
    let mut bind_idx = 1usize;

    enum BindVal {
        Str(String),
        I32(i32),
    }
    let mut binds: Vec<BindVal> = vec![];

    if let Some(ref city) = filters.city {
        conditions.push(format!("l.city ILIKE ${}", bind_idx));
        binds.push(BindVal::Str(format!("%{}%", city)));
        bind_idx += 1;
    }
    if let Some(ref pt) = filters.property_type {
        conditions.push(format!("l.property_type = ${}", bind_idx));
        binds.push(BindVal::Str(pt.clone()));
        bind_idx += 1;
    }
    if let Some(ref lt) = filters.listing_type {
        conditions.push(format!("l.listing_type = ${}", bind_idx));
        binds.push(BindVal::Str(lt.clone()));
        bind_idx += 1;
    }
    if let Some(ref ut) = filters.user_type {
        conditions.push(format!("l.user_type = ${}", bind_idx));
        binds.push(BindVal::Str(ut.clone()));
        bind_idx += 1;
    }
    if let Some(min) = filters.min_price {
        conditions.push(format!("l.price >= ${}", bind_idx));
        binds.push(BindVal::I32(min));
        bind_idx += 1;
    }
    if let Some(max) = filters.max_price {
        conditions.push(format!("l.price <= ${}", bind_idx));
        binds.push(BindVal::I32(max));
        bind_idx += 1;
    }
    if let Some(br) = filters.bedrooms {
        conditions.push(format!("l.bedrooms = ${}", bind_idx));
        binds.push(BindVal::I32(br));
        bind_idx += 1;
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let limit_pos = bind_idx;
    let offset_pos = bind_idx + 1;

    let sql = format!(
        r#"
        SELECT
            l.id, l.title,
            l.property_type, l.listing_type, l.user_type,
            l.price, l.deposit,
            l.location, l.city, l.area_sqft,
            l.bedrooms, l.bathrooms, l.parking,
            l.created_at,
            (SELECT COUNT(*) FROM listing_images_v2 li WHERE li.listing_id = l.id) AS image_count
        FROM listings_v2 l
        {}
        ORDER BY {}
        LIMIT ${} OFFSET ${}
        "#,
        where_clause, order_by, limit_pos, offset_pos
    );

    let count_sql = format!(
        "SELECT COUNT(*) FROM listings_v2 l {}",
        where_clause
    );

    // Build data query
    let mut q = sqlx::query(&sql);
    for b in &binds {
        q = match b {
            BindVal::Str(s) => q.bind(s.clone()),
            BindVal::I32(n) => q.bind(*n),
        };
    }
    q = q.bind(limit).bind(offset);

    let rows = match q.fetch_all(&app_state.db).await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": format!("Database error: {}", e)})),
            );
        }
    };

    // Build count query
    let mut cq = sqlx::query_scalar::<_, i64>(&count_sql);
    for b in &binds {
        cq = match b {
            BindVal::Str(s) => cq.bind(s.clone()),
            BindVal::I32(n) => cq.bind(*n),
        };
    }
    let total: i64 = cq.fetch_one(&app_state.db).await.unwrap_or(0);

    let listings: Vec<ListingSummary> = rows
        .iter()
        .map(|r| ListingSummary {
            id: r
                .try_get::<Uuid, _>("id")
                .map(|u| u.to_string())
                .unwrap_or_default(),
            title: r.try_get("title").unwrap_or_default(),
            property_type: r.try_get("property_type").unwrap_or_default(),
            listing_type: r.try_get("listing_type").unwrap_or_default(),
            user_type: r.try_get("user_type").unwrap_or_default(),
            price: r.try_get("price").unwrap_or_default(),
            deposit: r.try_get("deposit").unwrap_or_default(),
            location: r.try_get("location").unwrap_or_default(),
            city: r.try_get("city").ok(),
            area_sqft: r.try_get("area_sqft").unwrap_or_default(),
            bedrooms: r.try_get("bedrooms").ok(),
            bathrooms: r.try_get("bathrooms").ok(),
            parking: r.try_get("parking").ok(),
            created_at: r
                .try_get::<Option<chrono::DateTime<Utc>>, _>("created_at")
                .ok()
                .flatten()
                .map(|d| d.to_rfc3339()),
            image_count: r.try_get("image_count").unwrap_or(0),
        })
        .collect();

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": {
                "listings": listings,
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

// ─────────────────────────────────────────────────────────────────────────────
// 4. POST /api/listings/upload/images — Image Upload (S3)
// ─────────────────────────────────────────────────────────────────────────────

pub async fn upload_listing_images_v2(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    mut multipart: Multipart,
) -> impl IntoResponse {
    log::info!(
        "=== upload_listing_images_v2 called by user: {} ===",
        auth.user_id
    );

    let mut files: Vec<(String, String, bytes::Bytes, i64)> = Vec::new();
    let mut listing_id_str: Option<String> = None;

    // ── Parse multipart ─────────────────────────────────────────────────────
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        let file_name = field.file_name().map(|s| s.to_string());
        let content_type_header = field.content_type().map(|s| s.to_string());

        if name == "listing_id" {
            if let Ok(text) = field.text().await {
                listing_id_str = Some(text);
            }
            continue;
        }

        // Accept file fields
        if file_name.is_some()
            || name == "files"
            || name == "images"
            || name.starts_with("file")
            || name.starts_with("image")
        {
            // Max 10 images
            if files.len() >= 10 {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"success": false, "message": "Max 10 images allowed"})),
                )
                    .into_response();
            }

            let filename = file_name.unwrap_or_else(|| "upload.jpg".to_string());
            let content_type = content_type_header.unwrap_or_else(|| "image/jpeg".to_string());

            // Validate format
            if !content_type.starts_with("image/jpeg")
                && !content_type.starts_with("image/png")
                && !content_type.starts_with("image/webp")
            {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "success": false,
                        "message": format!("Only JPEG, PNG, and WEBP supported. Got: {}", content_type)
                    })),
                )
                    .into_response();
            }

            match field.bytes().await {
                Ok(bytes) => {
                    let size = bytes.len();
                    // 5MB per-file limit
                    if size > 5 * 1024 * 1024 {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({
                                "success": false,
                                "message": format!("File {} exceeds 5MB limit", filename)
                            })),
                        )
                            .into_response();
                    }
                    if size == 0 {
                        continue;
                    }
                    files.push((filename, content_type, bytes, size as i64));
                }
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({
                            "success": false,
                            "message": format!("Error reading file: {}", e)
                        })),
                    )
                        .into_response();
                }
            }
        } else {
            // Consume unknown fields
            let _ = field.bytes().await;
        }
    }

    if files.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "No files uploaded"})),
        )
            .into_response();
    }

    // Parse listing_id if provided (optional — images can be uploaded before listing creation)
    let listing_uuid = listing_id_str
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());

    let session_id = Uuid::new_v4().to_string();
    let mut uploaded_urls: Vec<serde_json::Value> = Vec::new();

    for (i, (filename, content_type, bytes, size)) in files.into_iter().enumerate() {
        let image_id = Uuid::new_v4();
        let key = format!("listings/v2/{}/{}_{}", session_id, image_id, filename);

        // Upload to S3
        match app_state
            .public_storage_service
            .upload_file(&key, bytes.to_vec(), &content_type)
            .await
        {
            Ok(()) => {
                log::info!("S3 upload OK: {}", key);
            }
            Err(e) => {
                log::error!("S3 upload FAILED: {}: {}", key, e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "message": format!("S3 upload failed for {}: {}", filename, e)
                    })),
                )
                    .into_response();
            }
        }

        let bucket_name = std::env::var("PUBLIC_BUCKET_NAME")
            .unwrap_or_else(|_| "livana-public-listings".to_string());
        let aws_region =
            std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let url = format!(
            "https://{}.s3.{}.amazonaws.com/{}",
            bucket_name, aws_region, key
        );

        let order = (i + 1) as i32;
        let now = Utc::now();

        // If listing_id provided, insert directly into listing_images_v2
        if let Some(lid) = listing_uuid {
            let _ = sqlx::query(
                r#"
                INSERT INTO listing_images_v2 (id, listing_id, image_url, display_order, created_at)
                VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(image_id)
            .bind(lid)
            .bind(&url)
            .bind(order)
            .bind(now)
            .execute(&app_state.db)
            .await;
        }

        uploaded_urls.push(json!({
            "image_id": image_id.to_string(),
            "url": url,
            "filename": filename,
            "size": size,
            "mime_type": content_type,
            "order": order,
            "uploaded_at": now.to_rfc3339(),
        }));
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": {
                "uploaded_images": uploaded_urls,
                "total_uploaded": uploaded_urls.len(),
                "session_id": session_id,
                "listing_id": listing_id_str
            }
        })),
    )
        .into_response()
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. POST /api/listings/drafts — Save Listing Draft
// ─────────────────────────────────────────────────────────────────────────────

pub async fn save_listing_draft(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Json(payload): Json<ListingDraftPayload>,
) -> impl IntoResponse {
    let user_id = match Uuid::parse_str(&auth.user_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"success": false, "message": "Invalid user ID in token"})),
            );
        }
    };

    let draft_id = Uuid::new_v4();
    let data_json = payload.data;

    let result = sqlx::query(
        r#"
        INSERT INTO listing_drafts (id, user_id, data)
        VALUES ($1, $2, $3)
        RETURNING id
        "#,
    )
    .bind(draft_id)
    .bind(user_id)
    .bind(&data_json)
    .fetch_one(&app_state.db)
    .await;

    match result {
        Ok(row) => {
            let id: Uuid = row.get("id");
            (
                StatusCode::CREATED,
                Json(json!({
                    "success": true,
                    "data": {
                        "draft_id": id.to_string()
                    }
                })),
            )
        }
        Err(e) => {
            log::error!("Failed to save listing draft: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": format!("Database error: {}", e)
                })),
            )
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. GET /api/listings/drafts — Get Listing Drafts
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_listing_drafts(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
) -> impl IntoResponse {
    let user_id = match Uuid::parse_str(&auth.user_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"success": false, "message": "Invalid user ID in token"})),
            );
        }
    };

    let rows = match sqlx::query(
        r#"
        SELECT id, user_id, data, created_at, updated_at
        FROM listing_drafts
        WHERE user_id = $1
        ORDER BY updated_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(&app_state.db)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": format!("Database error: {}", e)})),
            );
        }
    };

    let drafts: Vec<ListingDraftResponse> = rows
        .iter()
        .map(|r| ListingDraftResponse {
            id: r.try_get::<Uuid, _>("id").unwrap_or_default().to_string(),
            user_id: r.try_get::<Uuid, _>("user_id").unwrap_or_default().to_string(),
            data: r.try_get("data").unwrap_or_else(|_| json!({})),
            created_at: r
                .try_get::<Option<chrono::DateTime<Utc>>, _>("created_at")
                .ok()
                .flatten()
                .map(|d| d.to_rfc3339()),
            updated_at: r
                .try_get::<Option<chrono::DateTime<Utc>>, _>("updated_at")
                .ok()
                .flatten()
                .map(|d| d.to_rfc3339()),
        })
        .collect();

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": drafts
        })),
    )
}

