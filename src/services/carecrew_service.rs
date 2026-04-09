use crate::models::carecrew::{is_valid_status, is_valid_transition};
use crate::repository::carecrew_repository as repo;
use serde_json::{Value, json};
/// CareCrew Service Layer
/// Business logic for the CareCrew module — delegates to repository,
/// maps raw rows to JSON, and enforces domain rules (booking transitions, validation).
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;

// ─── Row mappers ──────────────────────────────────────────────────────────────

fn service_row_to_json(row: &sqlx::postgres::PgRow) -> Value {
    json!({
        "id":          row.try_get::<Uuid,_>("id").map(|u|u.to_string()).unwrap_or_default(),
        "name":        row.try_get::<String,_>("name").unwrap_or_default(),
        "description": row.try_get::<Option<String>,_>("description").unwrap_or_default(),
        "iconUrl":     row.try_get::<Option<String>,_>("icon_url").unwrap_or_default(),
        "category":    row.try_get::<Option<String>,_>("category").unwrap_or_default(),
        "isActive":    row.try_get::<bool,_>("is_active").unwrap_or(true),
    })
}

fn provider_row_to_json(row: &sqlx::postgres::PgRow) -> Value {
    json!({
        "id":          row.try_get::<Uuid,_>("id").map(|u|u.to_string()).unwrap_or_default(),
        "name":        row.try_get::<String,_>("name").unwrap_or_default(),
        "bio":         row.try_get::<Option<String>,_>("bio").unwrap_or_default(),
        "serviceType": row.try_get::<String,_>("service_type").unwrap_or_default(),
        "city":        row.try_get::<Option<String>,_>("city").unwrap_or_default(),
        "rating":      row.try_get::<f64,_>("rating").unwrap_or(0.0),
        "reviewCount": row.try_get::<i32,_>("review_count").unwrap_or(0),
        "isFeatured":  row.try_get::<bool,_>("is_featured").unwrap_or(false),
        "avatarUrl":   row.try_get::<Option<String>,_>("avatar_url").unwrap_or_default(),
        "phone":       row.try_get::<Option<String>,_>("phone").unwrap_or_default(),
        "isActive":    row.try_get::<bool,_>("is_active").unwrap_or(true),
    })
}

fn booking_row_to_json(row: &sqlx::postgres::PgRow) -> Value {
    json!({
        "id":          row.try_get::<Uuid,_>("id").map(|u|u.to_string()).unwrap_or_default(),
        "providerId":  row.try_get::<Uuid,_>("provider_id").map(|u|u.to_string()).unwrap_or_default(),
        "serviceId":   row.try_get::<Uuid,_>("service_id").map(|u|u.to_string()).unwrap_or_default(),
        "userId":      row.try_get::<Uuid,_>("user_id").map(|u|u.to_string()).unwrap_or_default(),
        "scheduledAt": row.try_get::<chrono::DateTime<chrono::Utc>,_>("scheduled_at")
                          .map(|d| d.to_rfc3339()).unwrap_or_default(),
        "status":      row.try_get::<String,_>("status").unwrap_or_default(),
        "notes":       row.try_get::<Option<String>,_>("notes").unwrap_or_default(),
        "createdAt":   row.try_get::<chrono::DateTime<chrono::Utc>,_>("created_at")
                          .map(|d| d.to_rfc3339()).unwrap_or_default(),
    })
}

fn booking_with_user_to_json(row: &sqlx::postgres::PgRow) -> Value {
    let mut base = booking_row_to_json(row);
    base["user"] = json!({
        "name":  format!("{} {}",
            row.try_get::<Option<String>,_>("first_name").unwrap_or_default().unwrap_or_default(),
            row.try_get::<Option<String>,_>("last_name").unwrap_or_default().unwrap_or_default()
        ).trim().to_string(),
        "email": row.try_get::<Option<String>,_>("email").unwrap_or_default(),
        "phone": row.try_get::<Option<String>,_>("phone_no").unwrap_or_default(),
    });
    base
}

// ─── Service APIs ─────────────────────────────────────────────────────────────

pub async fn list_services(db: &Pool<Postgres>) -> Result<Value, sqlx::Error> {
    let rows = repo::list_services(db).await?;
    let services: Vec<Value> = rows.iter().map(service_row_to_json).collect();
    Ok(json!({ "services": services, "total": services.len() }))
}

pub async fn get_service_by_id(
    db: &Pool<Postgres>,
    id: Uuid,
) -> Result<Option<Value>, sqlx::Error> {
    let row = repo::get_service_by_id(db, id).await?;
    Ok(row.as_ref().map(service_row_to_json))
}

// ─── Provider APIs ────────────────────────────────────────────────────────────

pub struct ProviderListResult {
    pub providers: Vec<Value>,
    pub total_count: i64,
    pub current_page: i32,
    pub total_pages: i32,
}

pub async fn search_providers(
    db: &Pool<Postgres>,
    service_type: Option<&str>,
    city: Option<&str>,
    page: i32,
    limit: i32,
) -> Result<ProviderListResult, sqlx::Error> {
    let (rows, total) = tokio::try_join!(
        repo::search_providers(db, service_type, city, page, limit),
        repo::count_providers(db, service_type, city),
    )?;

    let providers: Vec<Value> = rows.iter().map(provider_row_to_json).collect();
    let total_pages = ((total as f64) / (limit as f64)).ceil() as i32;
    let total_pages = total_pages.max(1);

    Ok(ProviderListResult {
        providers,
        total_count: total,
        current_page: page,
        total_pages,
    })
}

pub async fn get_featured_providers(db: &Pool<Postgres>, limit: i32) -> Result<Value, sqlx::Error> {
    let rows = repo::get_featured_providers(db, limit).await?;
    let providers: Vec<Value> = rows.iter().map(provider_row_to_json).collect();
    Ok(json!({ "providers": providers, "total": providers.len() }))
}

pub async fn get_provider_by_id(
    db: &Pool<Postgres>,
    id: Uuid,
) -> Result<Option<Value>, sqlx::Error> {
    let row = repo::get_provider_by_id(db, id).await?;
    match row {
        Some(ref r) => {
            let base = provider_row_to_json(r);
            // Enrich with additional fields the Flutter app expects
            let enriched = json!({
                "id":             base["id"],
                "name":           base["name"],
                "service_type":   base["serviceType"],
                "rating":         base["rating"],
                "review_count":   base["reviewCount"],
                "location":       base["city"],
                "phone":          base["phone"],
                "email":          null,
                "profile_image":  base["avatarUrl"],
                "specialties":    [],
                "hourly_rate":    null,
                "experience":     null,
                "is_verified":    base["isFeatured"],
                "is_active":      base["isActive"],
                "is_saved":       false,
                "availability":   "available",
                "bio":            r.try_get::<Option<String>,_>("bio").unwrap_or_default(),
                "gallery":        [],
                "reviews":        [],
                "completed_jobs": 0,
                "response_time":  null,
                "available_slots": []
            });
            Ok(Some(enriched))
        }
        None => Ok(None),
    }
}

// ─── Booking APIs ─────────────────────────────────────────────────────────────

pub enum BookingCreateError {
    ProviderNotFound,
    ServiceNotFound,
    InvalidScheduledAt,
    DbError(sqlx::Error),
}

pub async fn create_booking(
    db: &Pool<Postgres>,
    mut provider_id: Uuid,
    mut service_id: Uuid,
    user_id: Uuid,
    scheduled_at: &str,
    notes: Option<&str>,
    address: Option<&str>,
    problem_description: Option<&str>,
    contact_number: Option<&str>,
    estimated_cost: Option<f64>,
) -> Result<Value, BookingCreateError> {
    // 1. Fetch provider fully to resolve correct primary key (in case it was user_id)
    let p_row_opt = repo::get_provider_by_id(db, provider_id)
        .await
        .map_err(BookingCreateError::DbError)?;
    
    let p_row = match p_row_opt {
        Some(row) => row,
        None => return Err(BookingCreateError::ProviderNotFound),
    };
    
    // Override provider_id with the actual provider's PK
    provider_id = p_row.try_get::<Uuid, _>("id").unwrap_or(provider_id);
    let service_type: String = p_row.try_get::<String, _>("service_type").unwrap_or_default();

    // 2. Check if the exact service_id exists first (this will lazily sync from the services table if needed)
    let s_exists = repo::service_exists(db, service_id)
        .await
        .map_err(BookingCreateError::DbError)?;

    if !s_exists {
        // Fallback: Resolve the matching service by name (to fix Flutter 404 Service Not Found issues for stale/bad uuids)
        if let Ok(Some(resolved_service_id)) = repo::resolve_service_by_name(db, &service_type).await {
            service_id = resolved_service_id;
        } else {
            return Err(BookingCreateError::ServiceNotFound);
        }
    }

    // 3. Validate the datetime format
    if chrono::DateTime::parse_from_rfc3339(scheduled_at).is_err() {
        return Err(BookingCreateError::InvalidScheduledAt);
    }

    let booking_id = Uuid::new_v4();
    let row = repo::create_booking(
        db,
        booking_id,
        provider_id,
        service_id,
        user_id,
        scheduled_at,
        notes,
        address,
        problem_description,
        contact_number,
        estimated_cost,
    )
    .await
    .map_err(BookingCreateError::DbError)?;

    Ok(booking_row_to_json(&row))
}

pub enum BookingUpdateError {
    BookingNotFound,
    InvalidStatus(String),
    InvalidTransition { from: String, to: String },
    DbError(sqlx::Error),
}

pub async fn update_booking_status(
    db: &Pool<Postgres>,
    booking_id: Uuid,
    new_status: &str,
    notes: Option<&str>,
    estimated_cost: Option<f64>,
) -> Result<Value, BookingUpdateError> {
    // Validate new status value
    if !is_valid_status(new_status) {
        return Err(BookingUpdateError::InvalidStatus(new_status.to_string()));
    }

    // Fetch current booking
    let existing = repo::get_booking_by_id(db, booking_id)
        .await
        .map_err(BookingUpdateError::DbError)?
        .ok_or(BookingUpdateError::BookingNotFound)?;

    let current_status: String = existing.try_get("status").unwrap_or_default();

    // Validate transition
    if !is_valid_transition(&current_status, new_status) {
        return Err(BookingUpdateError::InvalidTransition {
            from: current_status,
            to: new_status.to_string(),
        });
    }

    let row = repo::update_booking_status(db, booking_id, new_status, notes, estimated_cost)
        .await
        .map_err(BookingUpdateError::DbError)?;

    Ok(booking_row_to_json(&row))
}

pub async fn get_provider_bookings(
    db: &Pool<Postgres>,
    provider_id: Uuid,
    page: i32,
    limit: i32,
) -> Result<ProviderListResult, sqlx::Error> {
    let (rows, total) = tokio::try_join!(
        repo::get_bookings_for_provider(db, provider_id, page, limit),
        repo::count_bookings_for_provider(db, provider_id),
    )?;

    let bookings: Vec<Value> = rows.iter().map(booking_with_user_to_json).collect();
    let total_pages = ((total as f64) / (limit as f64)).ceil() as i32;
    let total_pages = total_pages.max(1);

    Ok(ProviderListResult {
        providers: bookings,
        total_count: total,
        current_page: page,
        total_pages,
    })
}

// ──────────────────────────────────────────────────────────────────────────────
// New Endpoint Implementations
// ──────────────────────────────────────────────────────────────────────────────

pub async fn get_user_bookings(
    db: &Pool<Postgres>,
    user_id: Uuid,
    status: Option<&str>,
    page: i32,
    limit: i32,
) -> Result<Value, sqlx::Error> {
    let (bookings, total_count) = repo::get_user_bookings(db, user_id, status, page, limit).await?;
    
    let total_pages = ((total_count as f64) / (limit as f64)).ceil() as i32;
    let total_pages = total_pages.max(1);

    Ok(json!({
        "bookings": bookings,
        "total_count": total_count,
        "current_page": page,
        "total_pages": total_pages,
    }))
}

pub async fn get_provider_bookings_v2(
    db: &Pool<Postgres>,
    provider_id: Uuid,
    status: Option<&str>,
    page: i32,
    limit: i32,
) -> Result<Value, sqlx::Error> {
    let (bookings, total_count) = repo::get_provider_bookings_v2(db, provider_id, status, page, limit).await?;
    
    let total_pages = ((total_count as f64) / (limit as f64)).ceil() as i32;
    let total_pages = total_pages.max(1);

    Ok(json!({
        "bookings": bookings,
        "total_count": total_count,
        "current_page": page,
        "total_pages": total_pages,
    }))
}

pub async fn get_booking_details(
    db: &Pool<Postgres>,
    booking_id: Uuid,
) -> Result<Option<crate::models::carecrew::BookingDetailsResponse>, sqlx::Error> {
    repo::get_booking_details(db, booking_id).await
}
