use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;
use chrono::Utc;



use crate::{
    app_state::AppState,
    dtos::{
        pings::{
            ClosePingRequest, CreatePingRequest, MatchingPingQuery, PingDto, PingQuery,
            PingResponseDto, RespondPingRequest, RespondPingResponseDto,
        },
        response::ApiResponse,
    },
    models::pings::{Ping, PingResponseJoined},
    utils::{api_error::ApiError, auth_extractor::AuthenticationUser},
};



// Map Ping model to PingDto
fn map_ping_to_dto(ping: Ping) -> PingDto {
    PingDto {
        id: ping.id,
        user_id: ping.user_id,
        location: ping.location,
        latitude: ping.latitude,
        longitude: ping.longitude,
        property_type: ping.property_type,
        listing_type: ping.listing_type,
        min_budget: ping.min_budget,
        max_budget: ping.max_budget,
        min_bedrooms: ping.min_bedrooms,
        max_bedrooms: ping.max_bedrooms,
        note: ping.note,
        status: ping.status,
        close_reason: ping.close_reason,
        created_at: ping.created_at,
        updated_at: ping.updated_at,
    }
}

// POST /api/v1/pings
pub async fn create_ping(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Json(payload): Json<CreatePingRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".into()))?;



    let ping = sqlx::query_as::<_, Ping>(
        r#"
        INSERT INTO pings (
            user_id, location, latitude, longitude, property_type, listing_type,
            min_budget, max_budget, min_bedrooms, max_bedrooms, note
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING *
        "#,
    )
    .bind(user_id)
    .bind(&payload.location)
    .bind(payload.latitude)
    .bind(payload.longitude)
    .bind(&payload.property_type)
    .bind(&payload.listing_type)
    .bind(payload.min_budget)
    .bind(payload.max_budget)
    .bind(payload.min_bedrooms)
    .bind(payload.max_bedrooms)
    .bind(&payload.note)
    .fetch_one(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    // Issue 49: Match brokers and insert notifications
    // Rule: only brokers whose operating_cities includes the ping's location (exact trim/lowercase) and have kyc_status = 'VERIFIED'
    let query = r#"
        SELECT user_id FROM broker_profiles
        WHERE kyc_status = 'VERIFIED'
          AND user_id <> $2
          AND EXISTS (
            SELECT 1 FROM unnest(operating_cities) c
            WHERE trim(c) <> '' AND lower(trim(c)) = lower(trim($1))
          )
    "#;

    let matched_brokers: Vec<Uuid> = sqlx::query_scalar(query)
        .bind(&payload.location)
        .bind(user_id)
        .fetch_all(&app_state.db)
        .await
        .unwrap_or_default();

    if !matched_brokers.is_empty() {
        let title = "New Property Request";
        let prop_type = payload.property_type.as_deref().unwrap_or("Property");
        let list_type = payload.listing_type.as_deref().unwrap_or("looking for");
        let msg = format!("A new {} request for {} in {} has been posted.", prop_type, list_type, payload.location);

        for broker_id in matched_brokers {
            let notification_id = Uuid::new_v4();
            let res = sqlx::query(
                r#"
                INSERT INTO notifications (id, user_id, title, message, type, related_entity_id, related_entity_type)
                VALUES ($1, $2, $3, $4, 'PING', $5, 'PING')
                "#
            )
            .bind(notification_id)
            .bind(broker_id)
            .bind(title)
            .bind(&msg)
            .bind(ping.id)
            .execute(&app_state.db)
            .await;

            if let Err(e) = res {
                eprintln!("Failed to insert ping notification for broker {}: {}", broker_id, e);
            }
        }
    }

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            message: "Ping created successfully".into(),
            data: map_ping_to_dto(ping),
        }),
    ))
}

// GET /api/v1/pings/mine
pub async fn get_my_pings(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Query(query): Query<PingQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".into()))?;

    let status_filter = match query.status.as_deref() {
        Some("all") | None => None,
        Some(s) => Some(s),
    };

    let pings = sqlx::query_as::<_, Ping>(
        r#"
        SELECT * FROM pings
        WHERE user_id = $1
          AND status != 'deleted'
          AND ($2::text IS NULL OR status = $2)
        ORDER BY created_at DESC
        "#,
    )
    .bind(user_id)
    .bind(status_filter)
    .fetch_all(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let dtos: Vec<PingDto> = pings.into_iter().map(map_ping_to_dto).collect();

    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            message: "Pings fetched successfully".into(),
            data: dtos,
        }),
    ))
}

// GET /api/v1/pings/{pingId}
pub async fn get_ping_detail(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(ping_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".into()))?;

    let ping = sqlx::query_as::<_, Ping>("SELECT * FROM pings WHERE id = $1 AND status != 'deleted'")
        .bind(ping_id)
        .fetch_optional(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let ping = match ping {
        Some(p) => p,
        None => return Err(ApiError::NotFound("Ping not found".into())),
    };

    // The owner can still view their own ping.
    // A broker whose cities don't include the ping's location (or KYC not verified) gets 404.
    if ping.user_id != user_id {
        let is_matching_broker: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM broker_profiles b, unnest(b.operating_cities) c
                WHERE b.user_id = $1 AND b.kyc_status = 'VERIFIED'
                  AND trim(c) <> '' AND lower(trim(c)) = lower(trim($2))
            )
            "#,
        )
        .bind(user_id)
        .bind(&ping.location)
        .fetch_one(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

        if !is_matching_broker {
            return Err(ApiError::NotFound("Ping not found".into()));
        }
    }

    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            message: "Ping fetched successfully".into(),
            data: map_ping_to_dto(ping),
        }),
    ))
}

// PATCH /api/v1/pings/{pingId}/close
pub async fn close_ping(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(ping_id): Path<Uuid>,
    Json(payload): Json<ClosePingRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".into()))?;

    let res = sqlx::query(
        "UPDATE pings SET status = 'closed', close_reason = $1, updated_at = NOW() WHERE id = $2 AND user_id = $3 AND status = 'active'"
    )
    .bind(&payload.reason)
    .bind(ping_id)
    .bind(user_id)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    if res.rows_affected() == 0 {
        return Err(ApiError::NotFound("Active ping not found or unauthorized".into()));
    }

    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            message: "Ping closed successfully".into(),
            data: (),
        }),
    ))
}

// DELETE /api/v1/pings/{pingId}
pub async fn delete_ping(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(ping_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".into()))?;

    let res = sqlx::query(
        "UPDATE pings SET status = 'deleted', updated_at = NOW() WHERE id = $1 AND user_id = $2 AND status != 'deleted'"
    )
    .bind(ping_id)
    .bind(user_id)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    if res.rows_affected() == 0 {
        return Err(ApiError::NotFound("Ping not found or unauthorized".into()));
    }

    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            message: "Ping deleted successfully".into(),
            data: (),
        }),
    ))
}

// GET /api/v1/pings/matching
pub async fn get_matching_pings(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Query(query): Query<MatchingPingQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let broker_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".into()))?;

    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    let location_filter = query
        .location
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let property_type_filter = query
        .property_type
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let listing_type_filter = query
        .listing_type
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let query_sql = r#"
        SELECT p.* FROM pings p
        WHERE p.status = 'active'
          AND p.user_id <> $1
          AND EXISTS (
            SELECT 1 FROM broker_profiles b, unnest(b.operating_cities) c
            WHERE b.user_id = $1 AND b.kyc_status = 'VERIFIED'
              AND trim(c) <> '' AND lower(trim(c)) = lower(trim(p.location))
          )
          AND ($2::text IS NULL OR p.property_type = $2)
          AND ($3::text IS NULL OR p.listing_type = $3)
          AND ($4::text IS NULL OR lower(trim(p.location)) = lower(trim($4)))
        ORDER BY p.created_at DESC
        LIMIT $5 OFFSET $6
    "#;

    let pings = sqlx::query_as::<_, Ping>(query_sql)
        .bind(broker_id)
        .bind(property_type_filter)
        .bind(listing_type_filter)
        .bind(location_filter)
        .bind(limit)
        .bind(offset)
        .fetch_all(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let dtos: Vec<PingDto> = pings.into_iter().map(map_ping_to_dto).collect();

    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            message: "Matching pings fetched successfully".into(),
            data: dtos,
        }),
    ))
}

// POST /api/v1/pings/{pingId}/respond
pub async fn respond_to_ping(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(ping_id): Path<Uuid>,
    Json(payload): Json<RespondPingRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let broker_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".into()))?;

    let ping = sqlx::query_as::<_, Ping>("SELECT * FROM pings WHERE id = $1 AND status = 'active'")
        .bind(ping_id)
        .fetch_optional(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let ping = match ping {
        Some(p) => p,
        None => return Err(ApiError::NotFound("Active ping not found".into())),
    };

    // The owner cannot respond to their own ping
    if ping.user_id == broker_id {
        return Err(ApiError::Forbidden("You cannot respond to your own ping".into()));
    }

    // A broker whose cities don't include the ping's location (or KYC not verified) gets 403
    let is_matching_broker: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM broker_profiles b, unnest(b.operating_cities) c
            WHERE b.user_id = $1 AND b.kyc_status = 'VERIFIED'
              AND trim(c) <> '' AND lower(trim(c)) = lower(trim($2))
        )
        "#,
    )
    .bind(broker_id)
    .bind(&ping.location)
    .fetch_one(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    if !is_matching_broker {
        return Err(ApiError::Forbidden("You are not authorized to respond to pings outside your operating cities or your KYC is not verified".into()));
    }

    // Check if response already exists
    let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM ping_responses WHERE ping_id = $1 AND broker_id = $2")
        .bind(ping_id)
        .bind(broker_id)
        .fetch_optional(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    if exists.is_some() {
        return Err(ApiError::BadRequest("You have already responded to this ping".into()));
    }

    // Begin transaction for creating chat thread
    let mut tx = app_state.db.begin().await.map_err(|e| ApiError::InternalServerError(format!("Tx error: {}", e)))?;

    // Create Chat Thread
    let chat_id: Uuid = sqlx::query_scalar("INSERT INTO chats (name) VALUES (NULL) RETURNING id")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("DB error: {}", e)))?;

    // Add Participants
    sqlx::query("INSERT INTO chat_participants (chat_id, user_id) VALUES ($1, $2), ($1, $3)")
        .bind(chat_id)
        .bind(broker_id)
        .bind(ping.user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("DB error: {}", e)))?;

    // Insert Message
    sqlx::query("INSERT INTO messages (chat_id, sender_id, content) VALUES ($1, $2, $3)")
        .bind(chat_id)
        .bind(broker_id)
        .bind(&payload.message)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("DB error: {}", e)))?;

    // Create Ping Response Record
    sqlx::query("INSERT INTO ping_responses (ping_id, broker_id, chat_id, message) VALUES ($1, $2, $3, $4)")
        .bind(ping_id)
        .bind(broker_id)
        .bind(chat_id)
        .bind(&payload.message)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("DB error: {}", e)))?;

    // Issue 50: Notify Requester
    let broker_name: Option<String> = sqlx::query_scalar("SELECT first_name || ' ' || last_name FROM users WHERE id = $1")
        .bind(broker_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("DB error: {}", e)))?;

    let broker_name = broker_name.unwrap_or_else(|| "A broker".to_string());
    let title = "New Response on your Ping";
    let msg = format!("{} has responded to your ping.", broker_name);

    let notification_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO notifications (id, user_id, title, message, type, related_entity_id, related_entity_type) VALUES ($1, $2, $3, $4, 'PING_RESPONSE', $5, 'CHAT')"
    )
    .bind(notification_id)
    .bind(ping.user_id)
    .bind(title)
    .bind(&msg)
    .bind(chat_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("DB error: {}", e)))?;

    tx.commit().await.map_err(|e| ApiError::InternalServerError(format!("Commit error: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            message: "Response sent successfully".into(),
            data: RespondPingResponseDto {
                chat_id,
                ping_id,
                broker_id,
                responded_at: Utc::now(),
            },
        }),
    ))
}

// GET /api/v1/pings/{pingId}/responses
pub async fn get_ping_responses(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(ping_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".into()))?;

    // Verify ownership
    let owner_id: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM pings WHERE id = $1 AND status != 'deleted'")
        .bind(ping_id)
        .fetch_optional(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    match owner_id {
        Some(oid) if oid == user_id => {},
        Some(_) => return Err(ApiError::Forbidden("You do not have permission to view these responses".into())),
        None => return Err(ApiError::NotFound("Ping not found".into())),
    }

    let responses = sqlx::query_as::<_, PingResponseJoined>(
        r#"
        SELECT
            pr.id, pr.broker_id,
            COALESCE(u.first_name || ' ' || u.last_name, 'Broker') as broker_name,
            pr.message, pr.chat_id, pr.responded_at
        FROM ping_responses pr
        JOIN users u ON pr.broker_id = u.id
        WHERE pr.ping_id = $1
        ORDER BY pr.responded_at DESC
        "#
    )
    .bind(ping_id)
    .fetch_all(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let dtos: Vec<PingResponseDto> = responses.into_iter().map(|r| PingResponseDto {
        id: r.id,
        broker_id: r.broker_id,
        broker_name: r.broker_name,
        message: r.message,
        chat_id: r.chat_id,
        responded_at: r.responded_at,
    }).collect();

    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            message: "Responses fetched successfully".into(),
            data: dtos,
        }),
    ))
}
