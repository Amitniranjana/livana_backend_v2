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
            ParticipantItem, RegisterExpoRequest, UpdateExpoRequest,
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
             organizer_id, banner_image, max_participants, created_at, lat, lng)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
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
    .bind(payload.lat)
    .bind(payload.lng)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to create expo event: {}", e)))?;

    // ── Notify nearby users (within 50km) ──
    if let (Some(lat), Some(lng)) = (payload.lat, payload.lng) {
        let nearby_users: Vec<Uuid> = sqlx::query_scalar(
            r#"
            SELECT id FROM users
            WHERE last_known_lat IS NOT NULL AND last_known_lng IS NOT NULL
            AND (6371 * acos(cos(radians($1)) * cos(radians(last_known_lat))
                 * cos(radians(last_known_lng) - radians($2))
                 + sin(radians($1)) * sin(radians(last_known_lat)))) < 50
            "#,
        )
        .bind(lat)
        .bind(lng)
        .fetch_all(&app_state.db)
        .await
        .unwrap_or_default();

        if !nearby_users.is_empty() {
            let title = format!("New Expo Event Nearby: {}", payload.title);
            let message = format!("An exciting new property expo '{}' is happening near you on {}!", payload.title, event_date.format("%Y-%m-%d"));
            
            for user_id in nearby_users {
                let _ = sqlx::query(
                    r#"
                    INSERT INTO notifications (user_id, title, message, type, is_read, related_entity_id, related_entity_type, created_at)
                    VALUES ($1, $2, $3, 'EXPO', false, $4, 'expo_events', NOW())
                    "#,
                )
                .bind(user_id)
                .bind(&title)
                .bind(&message)
                .bind(expo_id)
                .execute(&app_state.db)
                .await;
            }
        }
    }

    // ── Area-Based Notifications (match selected_area) ──
    let db_clone = app_state.db.clone();
    let expo_title = payload.title.clone();
    let location_clone = payload.location.clone();
    let expo_id_str = expo_id.to_string();
    let event_date_str = event_date.format("%Y-%m-%d").to_string();

    tokio::spawn(async move {
        let matching_users: Vec<Uuid> = sqlx::query_scalar(
            r#"
            SELECT id FROM users 
            WHERE selected_area IS NOT NULL 
            AND selected_area ILIKE $1
            "#,
        )
        .bind(format!("%{}%", location_clone))
        .fetch_all(&db_clone)
        .await
        .unwrap_or_default();

        for uid in matching_users {
            let _ = crate::utils::notification_chat_helper::create_notification(
                &db_clone,
                uid,
                &format!("New Expo in Your Area: {}", expo_title),
                &format!("An exciting new property expo '{}' is happening in your area on {}!", expo_title, event_date_str),
                "SYSTEM",
                Uuid::parse_str(&expo_id_str).ok(),
                Some("ExpoEvent"),
            )
            .await;
        }
    });

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
    let rows: Vec<(Uuid, String, String, chrono::NaiveDate, i32, Option<f64>, Option<f64>)> =
        if let Some(ref city) = params.city {
            let pattern = format!("%{}%", city);
            sqlx::query_as(
                r#"
                SELECT id, title, location, event_date, registered_count, lat, lng
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
                SELECT id, title, location, event_date, registered_count, lat, lng
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
            |(id, title, location, event_date, registered_count, lat, lng)| ExpoEventListItem {
                expo_id: id,
                title,
                location,
                event_date: event_date.to_string(),
                registered_count,
                lat,
                lng,
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
        Option<f64>,                   // lat
        Option<f64>,                   // lng
    )> = sqlx::query_as(
        r#"
        SELECT id, title, description, location, event_date,
               start_time, end_time, organizer_id, banner_image,
               max_participants, created_at, lat, lng
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
        lat,
        lng,
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
            lat,
            lng,
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

// ---------------------------------------------------------------------------
// PUT /api/expo/{expo_id} — Edit Expo Event (partial update)
// ---------------------------------------------------------------------------

pub async fn edit_expo(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(expo_id): Path<Uuid>,
    Json(payload): Json<UpdateExpoRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    // 1. Ownership check — only organizer can edit
    let organizer: Option<Uuid> =
        sqlx::query_scalar("SELECT organizer_id FROM expo_events WHERE id = $1")
            .bind(expo_id)
            .fetch_optional(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    match organizer {
        None => return Err(ApiError::NotFound("Expo event not found".to_string())),
        Some(oid) if oid != user_id => return Err(ApiError::access_denied()),
        _ => {}
    }

    // 2. Validate max_participants >= current registered count
    if let Some(max_p) = payload.max_participants {
        let current_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM expo_registrations WHERE expo_id = $1")
                .bind(expo_id)
                .fetch_one(&app_state.db)
                .await
                .unwrap_or(0);

        if (max_p as i64) < current_count {
            return Err(ApiError::BadRequest(format!(
                "max_participants ({}) cannot be less than current registered users ({})",
                max_p, current_count
            )));
        }
    }

    // 3. Parse and validate dates if provided
    let event_date: Option<chrono::NaiveDate> = match &payload.event_date {
        Some(d) => Some(
            chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| {
                ApiError::BadRequest("Invalid event_date format. Expected YYYY-MM-DD".to_string())
            })?,
        ),
        None => None,
    };

    let start_time: Option<chrono::NaiveTime> = match &payload.start_time {
        Some(t) => Some(chrono::NaiveTime::parse_from_str(t, "%H:%M").map_err(|_| {
            ApiError::BadRequest("Invalid start_time format. Expected HH:MM".to_string())
        })?),
        None => None,
    };

    let end_time: Option<chrono::NaiveTime> = match &payload.end_time {
        Some(t) => Some(chrono::NaiveTime::parse_from_str(t, "%H:%M").map_err(|_| {
            ApiError::BadRequest("Invalid end_time format. Expected HH:MM".to_string())
        })?),
        None => None,
    };

    // 4. Partial update via COALESCE
    sqlx::query(
        r#"
        UPDATE expo_events SET
            title            = COALESCE($2, title),
            description      = COALESCE($3, description),
            location         = COALESCE($4, location),
            event_date       = COALESCE($5, event_date),
            start_time       = COALESCE($6, start_time),
            end_time         = COALESCE($7, end_time),
            banner_image     = COALESCE($8, banner_image),
            max_participants = COALESCE($9, max_participants),
            lat              = COALESCE($10, lat),
            lng              = COALESCE($11, lng),
            updated_at       = NOW()
        WHERE id = $1
        "#,
    )
    .bind(expo_id)
    .bind(&payload.title)
    .bind(&payload.description)
    .bind(&payload.location)
    .bind(event_date)
    .bind(start_time)
    .bind(end_time)
    .bind(&payload.banner_image)
    .bind(payload.max_participants)
    .bind(payload.lat)
    .bind(payload.lng)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to update expo: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Expo event updated successfully".to_string(),
        data: serde_json::json!({ "expo_id": expo_id }),
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// GET /api/expo/mine — Get Expos created by the authenticated user
// ---------------------------------------------------------------------------

pub async fn get_my_expos(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Query(params): Query<ExpoListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let organizer_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user UUID".to_string()))?;

    let limit = params.limit.clamp(1, 100);
    let offset = params.offset.max(0);

    let rows: Vec<(Uuid, String, String, chrono::NaiveDate, i32, Option<f64>, Option<f64>)> = sqlx::query_as(
        r#"
        SELECT id, title, location, event_date, registered_count, lat, lng
        FROM expo_events
        WHERE organizer_id = $1
        ORDER BY event_date ASC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(organizer_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let events: Vec<ExpoEventListItem> = rows
        .into_iter()
        .map(
            |(id, title, location, event_date, registered_count, lat, lng)| ExpoEventListItem {
                expo_id: id,
                title,
                location,
                event_date: event_date.to_string(),
                registered_count,
                lat,
                lng,
            },
        )
        .collect();

    let response = ApiResponse {
        success: true,
        message: "My expo events fetched successfully".to_string(),
        data: ExpoEventsData { events },
    };

    Ok((StatusCode::OK, Json(response)))
}
