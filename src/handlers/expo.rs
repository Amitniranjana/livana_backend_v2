// src/handlers/expo.rs
//
// Property Expo Event System
//   API 1: POST /api/expo   — Create Expo Event (Admin/Builder)
//   API 2: GET  /api/expo   — Get All Expo Events

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
        expo::{
            CreateExpoRequest, CreateExpoResponseData,
            ExpoEventListItem, ExpoEventsData, ExpoListQuery,
        },
        response::ApiResponse,
    },
    utils::{api_error::ApiError, auth_extractor::AuthenticationUser},
};

// ---------------------------------------------------------------------------
// API 1: POST /api/expo — Create a new Expo Event
// ---------------------------------------------------------------------------

pub async fn create_expo(
    State(app_state): State<AppState>,
    _auth: AuthenticationUser,
    Json(payload): Json<CreateExpoRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // ── Validate required fields ──
    if payload.title.trim().is_empty() {
        return Err(ApiError::BadRequest("Title cannot be empty".to_string()));
    }
    if payload.description.trim().is_empty() {
        return Err(ApiError::BadRequest("Description cannot be empty".to_string()));
    }
    if payload.location.trim().is_empty() {
        return Err(ApiError::BadRequest("Location cannot be empty".to_string()));
    }

    // Parse date & time strings
    let event_date = chrono::NaiveDate::parse_from_str(&payload.event_date, "%Y-%m-%d")
        .map_err(|_| ApiError::BadRequest("Invalid event_date format. Expected YYYY-MM-DD".to_string()))?;

    let start_time = chrono::NaiveTime::parse_from_str(&payload.start_time, "%H:%M")
        .map_err(|_| ApiError::BadRequest("Invalid start_time format. Expected HH:MM".to_string()))?;

    let end_time = chrono::NaiveTime::parse_from_str(&payload.end_time, "%H:%M")
        .map_err(|_| ApiError::BadRequest("Invalid end_time format. Expected HH:MM".to_string()))?;

    let organizer_id = Uuid::parse_str(&payload.organizer_id)
        .map_err(|_| ApiError::BadRequest("Invalid organizer_id UUID".to_string()))?;

    if payload.max_participants <= 0 {
        return Err(ApiError::BadRequest("max_participants must be greater than 0".to_string()));
    }

    let expo_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    let banner_image = payload.banner_image.unwrap_or_default();

    // ── Insert into database ──
    sqlx::query(
        r#"
        INSERT INTO expo_events
            (id, title, description, location, event_date, start_time, end_time,
             organizer_id, banner_image, max_participants, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
    )
    .bind(expo_id)
    .bind(&payload.title)
    .bind(&payload.description)
    .bind(&payload.location)
    .bind(event_date)
    .bind(start_time)
    .bind(end_time)
    .bind(organizer_id)
    .bind(&banner_image)
    .bind(payload.max_participants)
    .bind(now)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to create expo event: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Expo event created successfully".to_string(),
        data: CreateExpoResponseData {
            expo_id,
            title: payload.title,
            created_at: now.to_rfc3339(),
        },
    };

    Ok((StatusCode::CREATED, Json(response)))
}

// ---------------------------------------------------------------------------
// API 2: GET /api/expo — Get All Expo Events (with pagination + city filter)
// ---------------------------------------------------------------------------

pub async fn get_all_expos(
    State(app_state): State<AppState>,
    _auth: AuthenticationUser,
    Query(params): Query<ExpoListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    // Clamp limit between 1..100
    let limit = params.limit.clamp(1, 100);
    let offset = params.offset.max(0);

    // Build the query dynamically based on whether city filter is provided
    let rows: Vec<(Uuid, String, String, chrono::NaiveDate, i32)> =
        if let Some(ref city) = params.city {
            let pattern = format!("%{}%", city);
            sqlx::query_as(
                r#"
                SELECT id, title, location, event_date, registered_count
                FROM expo_events
                WHERE location ILIKE $1
                ORDER BY event_date ASC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(&pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?
        } else {
            sqlx::query_as(
                r#"
                SELECT id, title, location, event_date, registered_count
                FROM expo_events
                ORDER BY event_date ASC
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?
        };

    let events: Vec<ExpoEventListItem> = rows
        .into_iter()
        .map(|(id, title, location, event_date, registered_count)| ExpoEventListItem {
            expo_id: id,
            title,
            location,
            event_date: event_date.to_string(),
            registered_count,
        })
        .collect();

    let response = ApiResponse {
        success: true,
        message: "Expo events fetched successfully".to_string(),
        data: ExpoEventsData { events },
    };

    Ok((StatusCode::OK, Json(response)))
}
