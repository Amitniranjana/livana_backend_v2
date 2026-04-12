// src/handlers/service_listing.rs
//
// Service Provider Listing APIs:
//   POST /api/services              — Add a Service
//   GET  /api/services              — Get All Services
//   GET  /api/services/providers    — Filter Providers by Service

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::{
        response::ApiResponse,
        service::{
            AddServiceRequest, ProviderItem, ProvidersFilterQuery, ProvidersListData, ServiceItem,
            ServiceResponse, ServicesListData, ServicesQuery,
        },
    },
    utils::{api_error::ApiError, auth_extractor::AuthenticationUser},
};

const VALID_CATEGORIES: &[&str] = &[
    "interior_designer",
    "packers_movers",
    "cleaning",
    "furniture_rental",
    "electrician",
    "plumber",
];

// ---------------------------------------------------------------------------
// POST /api/services — Add a Service
// ---------------------------------------------------------------------------

pub async fn add_service(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Json(body): Json<AddServiceRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let provider_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user token".to_string()))?;

    // Validate category
    if !VALID_CATEGORIES.contains(&body.category.as_str()) {
        return Err(ApiError::CustomError(
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid category. Must be one of: {}",
                VALID_CATEGORIES.join(", ")
            ),
            "VALIDATION_ERROR".to_string(),
        ));
    }

    // Validate price > 0
    if body.price <= 0 {
        return Err(ApiError::CustomError(
            StatusCode::BAD_REQUEST,
            "Price must be greater than 0".to_string(),
            "VALIDATION_ERROR".to_string(),
        ));
    }

    let service_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    sqlx::query(
        r#"
        INSERT INTO services (id, provider_id, service_name, category, price, description, experience, location, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(service_id)
    .bind(provider_id)
    .bind(&body.service_name)
    .bind(&body.category)
    .bind(body.price)
    .bind(&body.description)
    .bind(&body.experience)
    .bind(&body.location)
    .bind(now)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to add service: {}", e)))?;

    // Auto-sync into carecrew_providers so that CareCrew Provider APIs work seamlessly
    // IMPORTANT: we use the user's own ID as the carecrew_providers.id so that
    // GET /api/v1/carecrew/providers/{user_id} resolves correctly.
    let _ = sqlx::query(
        r#"
        INSERT INTO carecrew_providers (
            id, name, bio, service_type, city, user_id, is_active, phone
        )
        SELECT 
            $4,
            COALESCE(first_name, 'Provider'), 
            $1, 
            $2, 
            $3, 
            id, 
            true,
            phone_no
        FROM users
        WHERE id = $4
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(&body.description)
    .bind(&body.category)
    .bind(&body.location)
    .bind(provider_id)
    .execute(&app_state.db)
    .await
    .map_err(|e| {
        println!(
            "[Service Adding] Failed to auto-sync carecrew_providers: {}",
            e
        )
    });

    let response = ApiResponse {
        success: true,
        message: "Service added successfully".to_string(),
        data: ServiceResponse {
            service_id,
            provider_id,
            service_name: body.service_name,
            category: body.category,
            price: body.price,
            description: body.description,
            experience: body.experience,
            location: body.location,
            created_at: now,
        },
    };

    Ok((StatusCode::CREATED, Json(response)))
}

// ---------------------------------------------------------------------------
// GET /api/services — Get All Services (paginated)
// ---------------------------------------------------------------------------

pub async fn get_all_services(
    State(app_state): State<AppState>,
    _auth: AuthenticationUser,
    Query(params): Query<ServicesQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = params.limit.unwrap_or(10).min(100).max(1);
    let offset = params.offset.unwrap_or(0).max(0);

    // Get total count
    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM services")
        .fetch_one(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    // Get paginated rows
    let rows = sqlx::query(
        r#"
        SELECT id, provider_id, service_name, category, price, description, experience, location, created_at
        FROM services
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let services: Vec<ServiceItem> = rows
        .iter()
        .map(|r| ServiceItem {
            id: r.get("id"),
            provider_id: r.get("provider_id"),
            service_name: r.get("service_name"),
            category: r.get("category"),
            price: r.get("price"),
            description: r.get("description"),
            experience: r.get("experience"),
            location: r.get("location"),
            created_at: r.get("created_at"),
        })
        .collect();

    let total_pages = if total_count == 0 {
        0
    } else {
        (total_count as f64 / limit as f64).ceil() as i64
    };
    let current_page = (offset / limit) + 1;

    let response = ApiResponse {
        success: true,
        message: "Services retrieved successfully".to_string(),
        data: ServicesListData {
            services,
            total_count,
            current_page,
            total_pages,
        },
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// GET /api/services/providers — Filter Providers by Service
// ---------------------------------------------------------------------------

pub async fn filter_providers(
    State(app_state): State<AppState>,
    _auth: AuthenticationUser,
    Query(params): Query<ProvidersFilterQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let service_type = params.service_type.as_deref().unwrap_or("").trim();
    if service_type.is_empty() {
        return Err(ApiError::CustomError(
            StatusCode::BAD_REQUEST,
            "service_type is required".to_string(),
            "VALIDATION_ERROR".to_string(),
        ));
    }

    let limit = params.limit.unwrap_or(10).min(100).max(1);
    let offset = params.offset.unwrap_or(0).max(0);
    let sort_by = params.sort_by.as_deref().unwrap_or("rating");

    // Validate sort_by
    if !["rating", "price", "experience"].contains(&sort_by) {
        return Err(ApiError::CustomError(
            StatusCode::BAD_REQUEST,
            "sort_by must be one of: rating, price, experience".to_string(),
            "VALIDATION_ERROR".to_string(),
        ));
    }

    // Count total matching providers
    let total_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT s.provider_id)
        FROM services s
        WHERE LOWER(s.category) = LOWER($1)
        "#,
    )
    .bind(service_type)
    .fetch_one(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    // ORDER BY clause — safe because we control the string from a validated enum.
    let order_clause = match sort_by {
        "price" => "MIN(s.price) ASC",
        "experience" => "s.experience DESC",
        _ => "COALESCE(AVG(cr.rating), 0) DESC",
    };

    // We use format! only for the ORDER BY clause which cannot be parameterized.
    // All user-supplied values (service_type, limit, offset) remain parameterized.
    let query = format!(
        r#"
        SELECT
            u.id,
            CONCAT(COALESCE(u.first_name, ''), ' ', COALESCE(u.last_name, '')) AS name,
            s.category                                      AS service_type,
            COALESCE(AVG(cr.rating), 0.0)::FLOAT8           AS rating,
            COUNT(cr.id)                                    AS review_count,
            s.location,
            MIN(s.price)::FLOAT8                            AS hourly_rate,
            s.experience,
            COALESCE(u.verified, false)                     AS is_verified,
            'available'                                     AS availability
        FROM services s
        JOIN users u ON u.id = s.provider_id
        LEFT JOIN carecrew_reviews cr ON cr.provider_id = s.provider_id
        WHERE LOWER(s.category) = LOWER($1)
        GROUP BY u.id, u.first_name, u.last_name, s.category, s.location, s.experience, u.verified
        ORDER BY {order_clause}
        LIMIT $2 OFFSET $3
        "#
    );

    let rows = sqlx::query(&query)
        .bind(service_type)
        .bind(limit)
        .bind(offset)
        .fetch_all(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let providers: Vec<ProviderItem> = rows
        .iter()
        .map(|r| ProviderItem {
            id: r.get("id"),
            name: r.get("name"),
            service_type: r.get("service_type"),
            rating: r.get::<f64, _>("rating"),
            review_count: r.get("review_count"),
            location: r.get("location"),
            hourly_rate: r.get("hourly_rate"),
            experience: r.get("experience"),
            is_verified: r.get("is_verified"),
            availability: r.get("availability"),
            distance_km: None, // geolocation not in DB; placeholder
        })
        .collect();

    let total_pages = if total_count == 0 {
        0
    } else {
        (total_count as f64 / limit as f64).ceil() as i64
    };
    let current_page = (offset / limit) + 1;

    let response = ApiResponse {
        success: true,
        message: "Providers retrieved successfully".to_string(),
        data: ProvidersListData {
            providers,
            total_count,
            current_page,
            total_pages,
        },
    };

    Ok((StatusCode::OK, Json(response)))
}
