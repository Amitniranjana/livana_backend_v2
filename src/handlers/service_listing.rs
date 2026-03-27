// src/handlers/service_listing.rs
//
// Service Provider Listing APIs:
//   POST /api/services              — Add a Service
//   GET  /api/services              — Get All Services
//   GET  /api/services/providers    — Filter Providers by Service

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::{
        response::ApiResponse,
        service::{
            AddServiceRequest, ServiceResponseData, ServiceItem,
            ServicesListData, ServicesListQuery,
            ProvidersFilterQuery, ProviderItem, ProvidersListData,
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
    Json(payload): Json<AddServiceRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let provider_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user token".to_string()))?;

    // Validate required fields
    if payload.service_name.trim().is_empty() {
        return Err(ApiError::CustomError(
            StatusCode::BAD_REQUEST,
            "service_name is required".to_string(),
            "VALIDATION_ERROR".to_string(),
        ));
    }
    if payload.category.trim().is_empty() {
        return Err(ApiError::CustomError(
            StatusCode::BAD_REQUEST,
            "category is required".to_string(),
            "VALIDATION_ERROR".to_string(),
        ));
    }
    if !VALID_CATEGORIES.contains(&payload.category.as_str()) {
        return Err(ApiError::CustomError(
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid category. Must be one of: {}",
                VALID_CATEGORIES.join(", ")
            ),
            "VALIDATION_ERROR".to_string(),
        ));
    }
    if payload.description.trim().is_empty() {
        return Err(ApiError::CustomError(
            StatusCode::BAD_REQUEST,
            "description is required".to_string(),
            "VALIDATION_ERROR".to_string(),
        ));
    }
    if payload.experience.trim().is_empty() {
        return Err(ApiError::CustomError(
            StatusCode::BAD_REQUEST,
            "experience is required".to_string(),
            "VALIDATION_ERROR".to_string(),
        ));
    }
    if payload.location.trim().is_empty() {
        return Err(ApiError::CustomError(
            StatusCode::BAD_REQUEST,
            "location is required".to_string(),
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
    .bind(&payload.service_name)
    .bind(&payload.category)
    .bind(payload.price)
    .bind(&payload.description)
    .bind(&payload.experience)
    .bind(&payload.location)
    .bind(now)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to add service: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Service added successfully".to_string(),
        data: ServiceResponseData {
            service_id,
            provider_id,
            service_name: payload.service_name,
            category: payload.category,
            price: payload.price,
            description: payload.description,
            experience: payload.experience,
            location: payload.location,
            created_at: now.to_rfc3339(),
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
    Query(params): Query<ServicesListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = params.limit.unwrap_or(10).clamp(1, 100);
    let offset = params.offset.unwrap_or(0).max(0);

    // Get total count
    let total_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM services")
            .fetch_one(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    // Get paginated rows
    let rows: Vec<(Uuid, Uuid, String, String, i32, String, String, String, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
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
        .into_iter()
        .map(|(id, provider_id, service_name, category, price, description, experience, location, created_at)| {
            ServiceItem {
                id,
                provider_id,
                service_name,
                category,
                price,
                description,
                experience,
                location,
                created_at: created_at.to_rfc3339(),
            }
        })
        .collect();

    let current_page = (offset / limit) + 1;
    let total_pages = if total_count == 0 {
        0
    } else {
        (total_count + limit - 1) / limit
    };

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
    let service_type = params.service_type.as_deref().unwrap_or("");
    if service_type.is_empty() {
        return Err(ApiError::CustomError(
            StatusCode::BAD_REQUEST,
            "service_type query parameter is required".to_string(),
            "VALIDATION_ERROR".to_string(),
        ));
    }

    let sort_by = params.sort_by.as_deref().unwrap_or("rating");
    let limit = params.limit.unwrap_or(10).clamp(1, 100);
    let offset = params.offset.unwrap_or(0).max(0);

    let order_clause = match sort_by {
        "price" => "s.price ASC",
        "experience" => "s.experience DESC",
        _ => "p.rating DESC", // default: rating
    };

    // Count total matching providers
    let total_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT p.id)
        FROM carecrew_providers p
        WHERE LOWER(p.service_type) = LOWER($1) AND p.is_active = TRUE
        "#,
    )
    .bind(service_type)
    .fetch_one(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    // Build the query with dynamic ORDER BY (safe since we control the string)
    let query_str = format!(
        r#"
        SELECT
            p.id,
            p.name,
            p.service_type,
            COALESCE(p.rating, 0.0) as rating,
            COALESCE(p.review_count, 0) as review_count,
            COALESCE(p.city, '') as location,
            COALESCE(
                (SELECT s.price FROM services s WHERE s.provider_id = p.user_id AND LOWER(s.category) = LOWER($1) LIMIT 1),
                0
            ) as hourly_rate,
            COALESCE(
                (SELECT s.experience FROM services s WHERE s.provider_id = p.user_id AND LOWER(s.category) = LOWER($1) LIMIT 1),
                ''
            ) as experience,
            CASE WHEN p.is_active THEN TRUE ELSE FALSE END as is_verified,
            'available' as availability
        FROM carecrew_providers p
        WHERE LOWER(p.service_type) = LOWER($1) AND p.is_active = TRUE
        ORDER BY {}
        LIMIT $2 OFFSET $3
        "#,
        order_clause
    );

    let rows: Vec<(Uuid, String, String, f32, i32, String, i32, String, bool, String)> =
        sqlx::query_as(&query_str)
            .bind(service_type)
            .bind(limit)
            .bind(offset)
            .fetch_all(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let providers: Vec<ProviderItem> = rows
        .into_iter()
        .map(|(id, name, svc_type, rating, review_count, location, hourly_rate, experience, is_verified, availability)| {
            ProviderItem {
                id,
                name,
                service_type: svc_type,
                rating: f64::from(rating),
                review_count,
                location,
                hourly_rate: f64::from(hourly_rate),
                experience,
                is_verified,
                availability,
                distance_km: None, // geolocation not supported yet
            }
        })
        .collect();

    let current_page = (offset / limit) + 1;
    let total_pages = if total_count == 0 {
        0
    } else {
        (total_count + limit - 1) / limit
    };

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
