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
    // Improved matching logic: Match if the broker's operating city is contained within the requested location (case-insensitive)
    let query = String::from(
        "SELECT user_id FROM broker_profiles WHERE EXISTS (SELECT 1 FROM unnest(operating_cities) c WHERE $1 ILIKE '%' || c || '%') AND kyc_status = 'VERIFIED'",
    );

    let matched_brokers: Vec<Uuid> = sqlx::query_scalar(&query)
        .bind(&payload.location)
        .fetch_all(&app_state.db)
        .await
        .unwrap_or_default();

    if !matched_brokers.is_empty() {
        let title = "New Property Request";
        let prop_type = payload.property_type.as_deref().unwrap_or("Property");
        let list_type = payload.listing_type.as_deref().unwrap_or("looking for");
        let msg = format!("A new {} request for {} in {} has been posted.", prop_type, list_type, payload.location);

        for broker_id in matched_brokers {
            let res = sqlx::query(
                r#"
                INSERT INTO notifications (user_id, title, message, type, related_entity_id, related_entity_type)
                VALUES ($1, $2, $3, 'PING', $4, 'PING')
                "#
            )
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

    let mut sql = "SELECT * FROM pings WHERE user_id = $1 AND status != 'deleted'".to_string();

    if let Some(status) = &query.status {
        if status != "all" {
            sql.push_str(format!(" AND status = '{}'", status).as_str());
        }
    }
    sql.push_str(" ORDER BY created_at DESC");

    let pings = sqlx::query_as::<_, Ping>(&sql)
        .bind(user_id)
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
    let _user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".into()))?;

    // Assuming requester can always view, and broker can view (we might want to add strict broker matching feed check here if required)
    let ping = sqlx::query_as::<_, Ping>("SELECT * FROM pings WHERE id = $1 AND status != 'deleted'")
        .bind(ping_id)
        .fetch_optional(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    match ping {
        Some(p) => Ok((
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                message: "Ping fetched successfully".into(),
                data: map_ping_to_dto(p),
            }),
        )),
        None => Err(ApiError::NotFound("Ping not found".into())),
    }
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
    let _broker_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".into()))?;

    let mut sql = "SELECT * FROM pings WHERE status = 'active'".to_string();

    if let Some(loc) = &query.location {
        sql.push_str(format!(" AND location = '{}'", loc).as_str());
    }
    if let Some(pt) = &query.property_type {
        sql.push_str(format!(" AND property_type = '{}'", pt).as_str());
    }
    if let Some(lt) = &query.listing_type {
        sql.push_str(format!(" AND listing_type = '{}'", lt).as_str());
    }

    sql.push_str(" ORDER BY created_at DESC");

    if let Some(limit) = query.limit {
        sql.push_str(format!(" LIMIT {}", limit).as_str());
    }
    if let Some(offset) = query.offset {
        sql.push_str(format!(" OFFSET {}", offset).as_str());
    }

    let pings = sqlx::query_as::<_, Ping>(&sql)
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

    sqlx::query(
        "INSERT INTO notifications (user_id, title, message, type, related_entity_id, related_entity_type) VALUES ($1, $2, $3, 'PING_RESPONSE', $4, 'CHAT')"
    )
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
