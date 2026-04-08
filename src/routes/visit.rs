use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::models::visit::{
    BookVisitRequest, PropertyInfo, ProviderInfo, SiteVisitRow, UpdateVisitStatusRequest, VisitItem,
};
use crate::utils::auth_extractor::AuthenticationUser;

// ─────────────────────────────────────────────────────────
// HELPER: SiteVisitRow → VisitItem
// ─────────────────────────────────────────────────────────
fn row_to_visit_item(row: SiteVisitRow) -> VisitItem {
    VisitItem {
        visit_id: row.visit_id,
        property: PropertyInfo {
            id: row.property_id,
            title: row.property_title,
            location: row.property_location,
        },
        provider: ProviderInfo {
            id: row.provider_id,
            name: row.provider_name,
            profile_image: row.provider_image,
        },
        scheduled_date_time: row.scheduled_date_time,
        status: row.status,
        contact_number: row.contact_number,
        notes: row.notes,
        cancellation_reason: row.cancellation_reason,
        created_at: row.created_at,
    }
}

// Reusable SQL fragment for fetching visit rows with JOINs
const VISIT_SELECT_SQL: &str = r#"
    SELECT
        sv.id                                       AS visit_id,
        sv.property_id,
        p.title                                     AS property_title,
        COALESCE(p.locality, p.city)                AS property_location,
        sv.user_id,
        sv.provider_id,
        (u.first_name || ' ' || u.last_name)        AS provider_name,
        u.profile_picture                           AS provider_image,
        sv.scheduled_date_time,
        sv.status,
        sv.contact_number,
        sv.notes,
        sv.cancellation_reason,
        sv.created_at
    FROM site_visits sv
    JOIN properties p ON p.id = sv.property_id
    JOIN users u      ON u.id = sv.provider_id
"#;

// ─────────────────────────────────────────────────────────
// HANDLER 1: Book Site Visit
// POST /api/visits
// ─────────────────────────────────────────────────────────
pub async fn book_visit_handler(
    auth: AuthenticationUser,
    State(state): State<AppState>,
    Json(body): Json<BookVisitRequest>,
) -> impl IntoResponse {
    let user_id = match Uuid::parse_str(&auth.user_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "message": "Invalid token",
                    "error_code": "INVALID_TOKEN"
                })),
            )
                .into_response();
        }
    };

    // Check property exists
    let property_exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS (SELECT 1 FROM properties WHERE id = $1)")
            .bind(body.property_id)
            .fetch_one(&state.db)
            .await;

    match property_exists {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "message": "Property not found",
                    "error_code": "PROPERTY_NOT_FOUND"
                })),
            )
                .into_response();
        }
        Err(e) => {
            println!("DB error checking property: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Internal server error",
                    "error_code": "DATABASE_ERROR"
                })),
            )
                .into_response();
        }
    }

    // Check duplicate booking
    let duplicate = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM site_visits
            WHERE property_id = $1
              AND user_id = $2
              AND scheduled_date_time = $3
              AND status != 'cancelled'
        )
        "#,
    )
    .bind(body.property_id)
    .bind(user_id)
    .bind(body.scheduled_date_time)
    .fetch_one(&state.db)
    .await;

    match duplicate {
        Ok(true) => {
            return (
                StatusCode::CONFLICT,
                Json(json!({
                    "success": false,
                    "message": "You have already booked a visit for this property at the same time",
                    "error_code": "VISIT_ALREADY_EXISTS"
                })),
            )
                .into_response();
        }
        Ok(false) => {}
        Err(e) => {
            println!("DB error checking duplicate: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Internal server error",
                    "error_code": "DATABASE_ERROR"
                })),
            )
                .into_response();
        }
    }

    // Insert the visit
    let insert_result = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO site_visits (
            property_id, user_id, provider_id,
            scheduled_date_time, contact_number, notes, status
        )
        VALUES ($1, $2, $3, $4, $5, $6, 'pending')
        RETURNING id
        "#,
    )
    .bind(body.property_id)
    .bind(user_id)
    .bind(body.provider_id)
    .bind(body.scheduled_date_time)
    .bind(&body.contact_number)
    .bind(&body.notes)
    .fetch_one(&state.db)
    .await;

    let visit_id = match insert_result {
        Ok(id) => id,
        Err(e) => {
            println!("DB error inserting visit: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Failed to book visit",
                    "error_code": "DATABASE_ERROR"
                })),
            )
                .into_response();
        }
    };

    // ── Trigger notification + chat (best-effort, don't fail the API) ──
    {
        use crate::utils::notification_chat_helper::{
            create_chat_if_not_exists, create_notification, get_user_display_name,
        };

        let db = &state.db;
        let user_name = get_user_display_name(db, user_id)
            .await
            .unwrap_or_else(|_| "A user".to_string());
        let scheduled_str = body.scheduled_date_time.format("%d %b %Y, %I:%M %p").to_string();

        // Notify the provider (builder / landlord / broker)
        if let Err(e) = create_notification(
            db,
            body.provider_id,
            "New Visit Booking! 📅",
            &format!(
                "{} booked a site visit on {}",
                user_name, scheduled_str
            ),
            "BOOKING",
            Some(visit_id),
            Some("SITE_VISIT"),
        )
        .await
        {
            println!("[Visit] Failed to create notification: {}", e);
        }

        // Auto-create chat if not exists + insert initial message
        if let Err(e) = create_chat_if_not_exists(
            db,
            user_id,
            body.provider_id,
            &format!(
                "📅 {} booked a site visit for {}",
                user_name, scheduled_str
            ),
        )
        .await
        {
            println!("[Visit] Failed to create chat: {}", e);
        }
    }

    // Fetch the newly created visit
    let query = format!("{} WHERE sv.id = $1", VISIT_SELECT_SQL);
    let visit = sqlx::query_as::<_, SiteVisitRow>(&query)
        .bind(visit_id)
        .fetch_one(&state.db)
        .await;

    match visit {
        Ok(row) => (
            StatusCode::CREATED,
            Json(json!({
                "success": true,
                "message": "Visit booked successfully",
                "data": row_to_visit_item(row)
            })),
        )
            .into_response(),
        Err(e) => {
            println!("DB error fetching new visit: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Visit booked but failed to fetch details",
                    "error_code": "DATABASE_ERROR"
                })),
            )
                .into_response()
        }
    }
}

// ─────────────────────────────────────────────────────────
// HANDLER 2: Get All Visits (User ke)
// GET /api/visits
// ─────────────────────────────────────────────────────────
pub async fn get_visits_handler(
    auth: AuthenticationUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let user_id = match Uuid::parse_str(&auth.user_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "message": "Invalid token",
                    "error_code": "INVALID_TOKEN"
                })),
            )
                .into_response();
        }
    };

    let query = format!(
        "{} WHERE sv.user_id = $1 ORDER BY sv.scheduled_date_time DESC",
        VISIT_SELECT_SQL
    );
    let rows = sqlx::query_as::<_, SiteVisitRow>(&query)
        .bind(user_id)
        .fetch_all(&state.db)
        .await;

    match rows {
        Ok(data) => {
            let visits: Vec<VisitItem> = data.into_iter().map(row_to_visit_item).collect();
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "data": visits
                })),
            )
                .into_response()
        }
        Err(e) => {
            println!("DB error fetching visits: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Internal server error",
                    "error_code": "DATABASE_ERROR"
                })),
            )
                .into_response()
        }
    }
}

// ─────────────────────────────────────────────────────────
// HANDLER 3: Get Single Visit Detail
// GET /api/visits/{visit_id}
// ─────────────────────────────────────────────────────────
pub async fn get_visit_detail_handler(
    auth: AuthenticationUser,
    State(state): State<AppState>,
    Path(visit_id): Path<Uuid>,
) -> impl IntoResponse {
    let user_id = match Uuid::parse_str(&auth.user_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "message": "Invalid token",
                    "error_code": "INVALID_TOKEN"
                })),
            )
                .into_response();
        }
    };

    let query = format!(
        "{} WHERE sv.id = $1 AND (sv.user_id = $2 OR sv.provider_id = $2)",
        VISIT_SELECT_SQL
    );
    let row = sqlx::query_as::<_, SiteVisitRow>(&query)
        .bind(visit_id)
        .bind(user_id)
        .fetch_optional(&state.db)
        .await;

    match row {
        Ok(Some(data)) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "data": row_to_visit_item(data)
            })),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "message": "Visit not found",
                "error_code": "VISIT_NOT_FOUND"
            })),
        )
            .into_response(),
        Err(e) => {
            println!("DB error fetching visit detail: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Internal server error",
                    "error_code": "DATABASE_ERROR"
                })),
            )
                .into_response()
        }
    }
}

// ─────────────────────────────────────────────────────────
// HANDLER 4: Update Visit Status (Provider only)
// PUT /api/visits/{visit_id}/status
// ─────────────────────────────────────────────────────────
pub async fn update_visit_status_handler(
    auth: AuthenticationUser,
    State(state): State<AppState>,
    Path(visit_id): Path<Uuid>,
    Json(body): Json<UpdateVisitStatusRequest>,
) -> impl IntoResponse {
    let provider_id = match Uuid::parse_str(&auth.user_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "message": "Invalid token",
                    "error_code": "INVALID_TOKEN"
                })),
            )
                .into_response();
        }
    };

    // Validate status value
    let valid_statuses = ["confirmed", "completed", "cancelled"];
    if !valid_statuses.contains(&body.status.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "message": "Invalid status. Allowed: confirmed, completed, cancelled",
                "error_code": "INVALID_STATUS"
            })),
        )
            .into_response();
    }

    // Check visit exists and belongs to this provider
    let visit_check = sqlx::query_scalar::<_, Uuid>(
        "SELECT user_id FROM site_visits WHERE id = $1 AND provider_id = $2",
    )
    .bind(visit_id)
    .bind(provider_id)
    .fetch_optional(&state.db)
    .await;

    let original_user_id = match visit_check {
        Ok(Some(uid)) => uid,
        Ok(None) => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "success": false,
                    "message": "Visit not found or access denied",
                    "error_code": "ACCESS_DENIED"
                })),
            )
                .into_response();
        }
        Err(e) => {
            println!("DB error checking visit ownership: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Internal server error",
                    "error_code": "DATABASE_ERROR"
                })),
            )
                .into_response();
        }
    };

    // Update the status
    let now = Utc::now();
    let update_result = sqlx::query(
        r#"
        UPDATE site_visits
        SET status = $1,
            cancellation_reason = $2,
            updated_at = $3
        WHERE id = $4
        "#,
    )
    .bind(&body.status)
    .bind(&body.cancellation_reason)
    .bind(now)
    .bind(visit_id)
    .execute(&state.db)
    .await;

    match update_result {
        Ok(_) => {
            // Update action_status on the related notification (best-effort)
            let action_status = match body.status.as_str() {
                "confirmed" => Some("ACCEPTED"),
                "cancelled" => Some("REJECTED"),
                _ => None,
            };
            if let Some(status_val) = action_status {
                let _ = sqlx::query(
                    r#"
                    UPDATE notifications
                    SET action_status = $1
                    WHERE related_entity_id = $2
                      AND related_entity_type = 'SITE_VISIT'
                    "#,
                )
                .bind(status_val)
                .bind(visit_id)
                .execute(&state.db)
                .await
                .map_err(|e| println!("[Visit] Failed to update notification action_status: {}", e));
            }

            // Notify the user about the status update
            {
                use crate::utils::notification_chat_helper::create_notification;
                let title = match body.status.as_str() {
                    "confirmed" => "Visit Confirmed! ✅",
                    "cancelled" => "Visit Cancelled ❌",
                    "completed" => "Visit Completed 🏁",
                    _ => "Visit Status Update",
                };

                let msg = if body.status.as_str() == "cancelled" {
                    format!("Your site visit was cancelled. Reason: {}", body.cancellation_reason.as_deref().unwrap_or("Not provided"))
                } else {
                    format!("Your site visit status was updated to '{}'", body.status)
                };

                if let Err(e) = create_notification(
                    &state.db,
                    original_user_id,
                    title,
                    &msg,
                    "VISIT_STATUS_UPDATE",
                    Some(visit_id),
                    Some("SITE_VISIT"),
                ).await {
                    println!("[Visit] Failed to create notification for status update: {}", e);
                }
            }

            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": format!("Visit status updated to '{}'", body.status)
                })),
            )
                .into_response()
        }
        Err(e) => {
            println!("DB error updating visit status: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Failed to update status",
                    "error_code": "DATABASE_ERROR"
                })),
            )
                .into_response()
        }
    }
}

// ─────────────────────────────────────────────────────────
// ROUTES
// ─────────────────────────────────────────────────────────
pub fn api_visit_routes() -> Router<AppState> {
    Router::new()
        .route("/api/visits", post(book_visit_handler))
        .route("/api/visits", get(get_visits_handler))
        .route("/api/visits/{visit_id}", get(get_visit_detail_handler))
        .route(
            "/api/visits/{visit_id}/status",
            put(update_visit_status_handler),
        )
}
