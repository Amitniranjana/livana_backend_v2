// src/handlers/expo.rs
//
// Property Expo Event System
//   API 1: POST /api/expo                   — Create Expo Event (Admin/Builder)
//   API 2: GET  /api/expo                   — Get All Expo Events
//   API 3: GET  /api/expo/{expo_id}         — Expo Event Details
//   API 4: POST /api/expo/{expo_id}/register — Register for Expo
//   API 5: GET  /api/expo/{expo_id}/participants — Get Expo Participants (Admin)

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::{
        expo::{
            CreateExpoRequest, CreateExpoResponseData, ExpoDetailData, ExpoEventListItem,
            ExpoEventsData, ExpoListQuery, ExpoParticipantsData, ExpoParticipantsQuery,
            ParticipantItem, RegisterExpoRequest,
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
        return Err(ApiError::BadRequest(
            "Description cannot be empty".to_string(),
        ));
    }
    if payload.location.trim().is_empty() {
        return Err(ApiError::BadRequest("Location cannot be empty".to_string()));
    }

    // Parse date & time strings
    let event_date =
        chrono::NaiveDate::parse_from_str(&payload.event_date, "%Y-%m-%d").map_err(|_| {
            ApiError::BadRequest("Invalid event_date format. Expected YYYY-MM-DD".to_string())
        })?;

    let start_time =
        chrono::NaiveTime::parse_from_str(&payload.start_time, "%H:%M").map_err(|_| {
            ApiError::BadRequest("Invalid start_time format. Expected HH:MM".to_string())
        })?;

    let end_time = chrono::NaiveTime::parse_from_str(&payload.end_time, "%H:%M")
        .map_err(|_| ApiError::BadRequest("Invalid end_time format. Expected HH:MM".to_string()))?;

    let organizer_id = Uuid::parse_str(&payload.organizer_id)
        .map_err(|_| ApiError::BadRequest("Invalid organizer_id UUID".to_string()))?;

    if payload.max_participants <= 0 {
        return Err(ApiError::BadRequest(
            "max_participants must be greater than 0".to_string(),
        ));
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
        .map(
            |(id, title, location, event_date, registered_count)| ExpoEventListItem {
                expo_id: id,
                title,
                location,
                event_date: event_date.to_string(),
                registered_count,
            },
        )
        .collect();

    let response = ApiResponse {
        success: true,
        message: "Expo events fetched successfully".to_string(),
        data: ExpoEventsData { events },
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// API 3: GET /api/expo/{expo_id} — Expo Event Details
// ---------------------------------------------------------------------------

pub async fn get_expo_details(
    State(app_state): State<AppState>,
    _auth: AuthenticationUser,
    Path(expo_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    // Fetch the event row
    let row: Option<(
        Uuid,                          // id
        String,                        // title
        String,                        // description
        String,                        // location
        chrono::NaiveDate,             // event_date
        chrono::NaiveTime,             // start_time
        chrono::NaiveTime,             // end_time
        Uuid,                          // organizer_id
        String,                        // banner_image
        i32,                           // max_participants
        chrono::DateTime<chrono::Utc>, // created_at
    )> = sqlx::query_as(
        r#"
        SELECT id, title, description, location, event_date,
               start_time, end_time, organizer_id, banner_image,
               max_participants, created_at
        FROM expo_events
        WHERE id = $1
        "#,
    )
    .bind(expo_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let (
        id,
        title,
        description,
        location,
        event_date,
        start_time,
        end_time,
        organizer_id,
        banner_image,
        max_participants,
        created_at,
    ) = row.ok_or_else(|| ApiError::NotFound("Expo event not found".to_string()))?;

    // Get live participants count from expo_registrations
    let participants_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM expo_registrations WHERE expo_id = $1")
            .bind(id)
            .fetch_one(&app_state.db)
            .await
            .unwrap_or(0);

    // Mocked services_available (as per requirement — can be wired to a table later)
    let services_available = vec![
        "Interior Design".to_string(),
        "Plumber".to_string(),
        "Electrician".to_string(),
    ];

    let response = ApiResponse {
        success: true,
        message: "Expo event details retrieved successfully".to_string(),
        data: ExpoDetailData {
            expo_id: id,
            title,
            description,
            location,
            event_date: event_date.to_string(),
            start_time: start_time.format("%H:%M").to_string(),
            end_time: end_time.format("%H:%M").to_string(),
            organizer_id,
            banner_image,
            max_participants,
            participants_count,
            services_available,
            created_at: created_at.to_rfc3339(),
        },
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// API 4: POST /api/expo/{expo_id}/register — Register for Expo
// ---------------------------------------------------------------------------

pub async fn register_for_expo(
    State(app_state): State<AppState>,
    _auth: AuthenticationUser,
    Path(expo_id): Path<Uuid>,
    Json(payload): Json<RegisterExpoRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&payload.user_id)
        .map_err(|_| ApiError::BadRequest("Invalid user_id UUID".to_string()))?;

    if payload.user_type.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "user_type cannot be empty".to_string(),
        ));
    }

    // 1. Verify the expo exists and fetch max_participants
    let expo_row: Option<(i32,)> =
        sqlx::query_as("SELECT max_participants FROM expo_events WHERE id = $1")
            .bind(expo_id)
            .fetch_optional(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let (max_participants,) =
        expo_row.ok_or_else(|| ApiError::NotFound("Expo event not found".to_string()))?;

    // 2. Check current registration count
    let current_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM expo_registrations WHERE expo_id = $1")
            .bind(expo_id)
            .fetch_one(&app_state.db)
            .await
            .unwrap_or(0);

    if current_count >= max_participants as i64 {
        return Err(ApiError::Conflict(
            "Expo event is full. Maximum participants reached".to_string(),
        ));
    }

    // 3. Insert registration (ON CONFLICT handles duplicate user+expo)
    let result = sqlx::query(
        r#"
        INSERT INTO expo_registrations (id, expo_id, user_id, user_type, company_name)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (expo_id, user_id) DO NOTHING
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(expo_id)
    .bind(user_id)
    .bind(&payload.user_type)
    .bind(&payload.company_name)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to register: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::Conflict(
            "You are already registered for this expo event".to_string(),
        ));
    }

    let response = ApiResponse {
        success: true,
        message: "Registered successfully for expo".to_string(),
        data: serde_json::json!({}),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

// ---------------------------------------------------------------------------
// API 5: GET /api/expo/{expo_id}/participants — Get Expo Participants (Admin)
// ---------------------------------------------------------------------------

pub async fn get_expo_participants(
    State(app_state): State<AppState>,
    _auth: AuthenticationUser,
    Path(expo_id): Path<Uuid>,
    Query(params): Query<ExpoParticipantsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    // Clamp limit between 1..100, ensure offset >= 0
    let limit = params.limit.clamp(1, 100);
    let offset = params.offset.max(0);

    // 1. Verify the expo event exists
    let expo_exists: Option<(i32,)> =
        sqlx::query_as("SELECT max_participants FROM expo_events WHERE id = $1")
            .bind(expo_id)
            .fetch_optional(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    if expo_exists.is_none() {
        return Err(ApiError::NotFound("Expo event not found".to_string()));
    }

    // 2. Get total count (with optional user_type filter)
    let total_count: i64 = if let Some(ref user_type) = params.user_type {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM expo_registrations WHERE expo_id = $1 AND user_type = $2",
        )
        .bind(expo_id)
        .bind(user_type)
        .fetch_one(&app_state.db)
        .await
        .unwrap_or(0)
    } else {
        sqlx::query_scalar("SELECT COUNT(*) FROM expo_registrations WHERE expo_id = $1")
            .bind(expo_id)
            .fetch_one(&app_state.db)
            .await
            .unwrap_or(0)
    };

    // 3. Fetch paginated participant rows
    let rows: Vec<(
        Uuid,
        Uuid,
        String,
        Option<String>,
        chrono::DateTime<chrono::Utc>,
    )> = if let Some(ref user_type) = params.user_type {
        sqlx::query_as(
            r#"
                SELECT id, user_id, user_type, company_name, registered_at
                FROM expo_registrations
                WHERE expo_id = $1 AND user_type = $2
                ORDER BY registered_at DESC
                LIMIT $3 OFFSET $4
                "#,
        )
        .bind(expo_id)
        .bind(user_type)
        .bind(limit)
        .bind(offset)
        .fetch_all(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?
    } else {
        sqlx::query_as(
            r#"
                SELECT id, user_id, user_type, company_name, registered_at
                FROM expo_registrations
                WHERE expo_id = $1
                ORDER BY registered_at DESC
                LIMIT $2 OFFSET $3
                "#,
        )
        .bind(expo_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?
    };

    let participants: Vec<ParticipantItem> = rows
        .into_iter()
        .map(
            |(id, user_id, user_type, company_name, registered_at)| ParticipantItem {
                registration_id: id,
                user_id,
                user_type,
                company_name,
                registered_at: registered_at.to_rfc3339(),
            },
        )
        .collect();

    // 4. Compute pagination metadata
    let current_page = (offset / limit) + 1;
    let total_pages = if total_count == 0 {
        0
    } else {
        (total_count + limit - 1) / limit
    };

    let response = ApiResponse {
        success: true,
        message: "Participants retrieved successfully".to_string(),
        data: ExpoParticipantsData {
            participants,
            total_count,
            current_page,
            total_pages,
        },
    };

    Ok((StatusCode::OK, Json(response)))
}
