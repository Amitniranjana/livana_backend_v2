/// CareCrew Repository
/// Raw sqlx::query() functions for all CareCrew CRUD operations.
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;

// ──────────────────────────────────────────────────────────────────────────────
// Services
// ──────────────────────────────────────────────────────────────────────────────

pub async fn list_services(db: &Pool<Postgres>) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
    sqlx::query(
        "SELECT id, name, description, icon_url, category, is_active, created_at
         FROM carecrew_services
         WHERE is_active = true
         ORDER BY name ASC",
    )
    .fetch_all(db)
    .await
}

pub async fn get_service_by_id(
    db: &Pool<Postgres>,
    id: Uuid,
) -> Result<Option<sqlx::postgres::PgRow>, sqlx::Error> {
    sqlx::query(
        "SELECT id, name, description, icon_url, category, is_active, created_at
         FROM carecrew_services
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await
}

// ──────────────────────────────────────────────────────────────────────────────
// Providers
// ──────────────────────────────────────────────────────────────────────────────

pub async fn search_providers(
    db: &Pool<Postgres>,
    service_type: Option<&str>,
    city: Option<&str>,
    page: i32,
    limit: i32,
) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
    let offset = ((page - 1) * limit) as i64;

    match (service_type, city) {
        (Some(st), Some(c)) => sqlx::query(
            "SELECT id, name, bio, service_type, city, rating, review_count, is_featured, avatar_url, phone, created_at
             FROM carecrew_providers
             WHERE is_active = true AND service_type ILIKE $1 AND city ILIKE $2
             ORDER BY is_featured DESC, rating DESC
             LIMIT $3 OFFSET $4"
        )
        .bind(format!("%{}%", st))
        .bind(format!("%{}%", c))
        .bind(limit as i64)
        .bind(offset)
        .fetch_all(db)
        .await,

        (Some(st), None) => sqlx::query(
            "SELECT id, name, bio, service_type, city, rating, review_count, is_featured, avatar_url, phone, created_at
             FROM carecrew_providers
             WHERE is_active = true AND service_type ILIKE $1
             ORDER BY is_featured DESC, rating DESC
             LIMIT $2 OFFSET $3"
        )
        .bind(format!("%{}%", st))
        .bind(limit as i64)
        .bind(offset)
        .fetch_all(db)
        .await,

        (None, Some(c)) => sqlx::query(
            "SELECT id, name, bio, service_type, city, rating, review_count, is_featured, avatar_url, phone, created_at
             FROM carecrew_providers
             WHERE is_active = true AND city ILIKE $1
             ORDER BY is_featured DESC, rating DESC
             LIMIT $2 OFFSET $3"
        )
        .bind(format!("%{}%", c))
        .bind(limit as i64)
        .bind(offset)
        .fetch_all(db)
        .await,

        (None, None) => sqlx::query(
            "SELECT id, name, bio, service_type, city, rating, review_count, is_featured, avatar_url, phone, created_at
             FROM carecrew_providers
             WHERE is_active = true
             ORDER BY is_featured DESC, rating DESC
             LIMIT $1 OFFSET $2"
        )
        .bind(limit as i64)
        .bind(offset)
        .fetch_all(db)
        .await,
    }
}

pub async fn count_providers(
    db: &Pool<Postgres>,
    service_type: Option<&str>,
    city: Option<&str>,
) -> Result<i64, sqlx::Error> {
    let row = match (service_type, city) {
        (Some(st), Some(c)) => sqlx::query(
            "SELECT COUNT(*) as total FROM carecrew_providers WHERE is_active=true AND service_type ILIKE $1 AND city ILIKE $2"
        ).bind(format!("%{}%", st)).bind(format!("%{}%", c)).fetch_one(db).await?,
        (Some(st), None) => sqlx::query(
            "SELECT COUNT(*) as total FROM carecrew_providers WHERE is_active=true AND service_type ILIKE $1"
        ).bind(format!("%{}%", st)).fetch_one(db).await?,
        (None, Some(c)) => sqlx::query(
            "SELECT COUNT(*) as total FROM carecrew_providers WHERE is_active=true AND city ILIKE $1"
        ).bind(format!("%{}%", c)).fetch_one(db).await?,
        (None, None) => sqlx::query(
            "SELECT COUNT(*) as total FROM carecrew_providers WHERE is_active=true"
        ).fetch_one(db).await?,
    };
    Ok(row.get::<i64, _>("total"))
}

pub async fn get_featured_providers(
    db: &Pool<Postgres>,
    limit: i32,
) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
    sqlx::query(
        "SELECT id, name, bio, service_type, city, rating, review_count, is_featured, avatar_url, phone, created_at
         FROM carecrew_providers
         WHERE is_active = true AND is_featured = true
         ORDER BY rating DESC
         LIMIT $1"
    )
    .bind(limit as i64)
    .fetch_all(db)
    .await
}

// NOTE: Using raw query() instead of query_as!() because the WHERE clause
// uses OR condition (id OR user_id) which requires runtime flexibility.
// Compile-time check not possible here. Manually verified column names.
pub async fn get_provider_by_id(
    db: &Pool<Postgres>,
    id: Uuid,
) -> Result<Option<sqlx::postgres::PgRow>, sqlx::Error> {
    sqlx::query(
        "SELECT id, name, bio, service_type, city, rating, review_count, is_featured, avatar_url, phone, user_id, is_active, created_at
         FROM carecrew_providers
         WHERE id = $1 OR user_id = $1"
    )
    .bind(id)
    .fetch_optional(db)
    .await
}

// ──────────────────────────────────────────────────────────────────────────────
// Bookings
// ──────────────────────────────────────────────────────────────────────────────

pub async fn create_booking(
    db: &Pool<Postgres>,
    id: Uuid,
    provider_id: Uuid,
    service_id: Uuid,
    user_id: Uuid,
    scheduled_at: &str,
    notes: Option<&str>,
    address: Option<&str>,
    problem_description: Option<&str>,
    contact_number: Option<&str>,
    estimated_cost: Option<f64>,
) -> Result<sqlx::postgres::PgRow, sqlx::Error> {
    // Parse the scheduled_at string into a timestamptz
    let scheduled_at_ts = chrono::DateTime::parse_from_rfc3339(scheduled_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|_| sqlx::Error::Protocol("Invalid scheduled_at datetime format".into()))?;

    let booking_number = format!(
        "BKG{}{}",
        chrono::Utc::now().format("%Y%m%d"),
        &uuid::Uuid::new_v4().to_string()[0..6].to_uppercase()
    );

    let mut tx = db.begin().await?;

    let row = sqlx::query(
        r#"
        INSERT INTO carecrew_bookings (id, booking_number, provider_id, service_id, user_id, scheduled_at, status, notes, address, problem_description, contact_number, estimated_cost, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, 'pending', $7, $8, $9, $10, $11, NOW())
        RETURNING *
        "#
    )
    .bind(id)
    .bind(&booking_number)
    .bind(provider_id)
    .bind(service_id)
    .bind(user_id)
    .bind(scheduled_at_ts)
    .bind(notes)
    .bind(address)
    .bind(problem_description)
    .bind(contact_number)
    .bind(estimated_cost)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO carecrew_booking_tracking (booking_id, status, description)
        VALUES ($1, 'pending', 'Your booking has been placed successfully')
        "#,
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(row)
}

#[allow(dead_code)]
pub async fn get_booking_by_id(
    db: &Pool<Postgres>,
    id: Uuid,
) -> Result<Option<sqlx::postgres::PgRow>, sqlx::Error> {
    sqlx::query(
        "SELECT id, provider_id, service_id, user_id, scheduled_at, status, notes, created_at
         FROM carecrew_bookings WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await
}

pub async fn update_booking_status(
    db: &Pool<Postgres>,
    id: Uuid,
    new_status: &str,
    notes: Option<&str>,
    estimated_cost: Option<f64>,
) -> Result<sqlx::postgres::PgRow, sqlx::Error> {
    let mut tx = db.begin().await?;

    let mut query = String::from("UPDATE carecrew_bookings SET status = $1, updated_at = NOW()");
    if notes.is_some() {
        query.push_str(", notes = $3");
    }
    if estimated_cost.is_some() {
        query.push_str(if notes.is_some() {
            ", estimated_cost = $4"
        } else {
            ", estimated_cost = $3"
        });
    }
    query.push_str(" WHERE id = $2 RETURNING *");

    let mut q = sqlx::query(&query).bind(new_status).bind(id);
    if let Some(n) = notes {
        q = q.bind(n);
    }
    if let Some(c) = estimated_cost {
        q = q.bind(c);
    }

    let row = q.fetch_one(&mut *tx).await?;

    sqlx::query(
        r#"
        INSERT INTO carecrew_booking_tracking (booking_id, status, description)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(id)
    .bind(new_status)
    .bind(format!("Booking status updated to {}", new_status))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(row)
}

pub async fn get_bookings_for_provider(
    db: &Pool<Postgres>,
    provider_id: Uuid,
    page: i32,
    limit: i32,
) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
    let offset = ((page - 1) * limit) as i64;
    sqlx::query(
        r#"
        SELECT b.id, b.provider_id, b.service_id, b.user_id, b.scheduled_at,
               b.status, b.notes, b.created_at,
               u.first_name, u.last_name, u.email, u.phone_no
        FROM carecrew_bookings b
        LEFT JOIN users u ON b.user_id = u.id
        WHERE b.provider_id = $1
        ORDER BY b.scheduled_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(provider_id)
    .bind(limit as i64)
    .bind(offset)
    .fetch_all(db)
    .await
}

pub async fn count_bookings_for_provider(
    db: &Pool<Postgres>,
    provider_id: Uuid,
) -> Result<i64, sqlx::Error> {
    let row = sqlx::query("SELECT COUNT(*) as total FROM carecrew_bookings WHERE provider_id = $1")
        .bind(provider_id)
        .fetch_one(db)
        .await?;
    Ok(row.get::<i64, _>("total"))
}

#[allow(dead_code)]
pub async fn provider_exists(db: &Pool<Postgres>, provider_id: Uuid) -> Result<bool, sqlx::Error> {
    let row = sqlx::query(
        "SELECT COUNT(*) as total FROM carecrew_providers WHERE id = $1 AND is_active = true",
    )
    .bind(provider_id)
    .fetch_one(db)
    .await?;
    Ok(row.get::<i64, _>("total") > 0)
}

/// Checks if service exists and is active.
pub async fn service_exists(db: &Pool<Postgres>, service_id: Uuid) -> Result<bool, sqlx::Error> {
    // 1. Check if it exists in carecrew_services
    let row = sqlx::query(
        "SELECT COUNT(*) as total FROM carecrew_services WHERE id = $1 AND is_active = true",
    )
    .bind(service_id)
    .fetch_one(db)
    .await?;

    if row.get::<i64, _>("total") > 0 {
        return Ok(true);
    }

    // 2. Fallback: check if it exists in 'services'
    let service_row_opt = sqlx::query(
        "SELECT id, service_name, category, description FROM services WHERE id = $1 LIMIT 1",
    )
    .bind(service_id)
    .fetch_optional(db)
    .await?;

    if let Some(s_row) = service_row_opt {
        // Lazily sync this service to carecrew_services so FK constraints pass
        let _ = sqlx::query(
            "INSERT INTO carecrew_services (id, name, category, description, is_active) VALUES ($1, $2, $3, $4, true) ON CONFLICT DO NOTHING"
        )
        .bind(s_row.get::<Uuid, _>("id"))
        .bind(s_row.get::<String, _>("service_name"))
        .bind(s_row.get::<String, _>("category"))
        .bind(s_row.get::<String, _>("description"))
        .execute(db)
        .await;

        return Ok(true);
    }

    Ok(false)
}

/// Resolves a service ID by its name (service_type) to auto-correct mismatched UUIDs
pub async fn resolve_service_by_name(
    db: &Pool<Postgres>,
    service_name: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id FROM carecrew_services WHERE name ILIKE $1 AND is_active = true LIMIT 1",
    )
    .bind(service_name)
    .fetch_optional(db)
    .await?;
    Ok(row.map(|r| r.get::<Uuid, _>("id")))
}

// ──────────────────────────────────────────────────────────────────────────────
// Endpoints 33, 34, 35 Implementation
// ──────────────────────────────────────────────────────────────────────────────

use crate::models::carecrew::{ProviderBookingResponse, UserBookingResponse};

pub async fn get_user_bookings(
    db: &Pool<Postgres>,
    user_id: Uuid,
    status: Option<&str>,
    page: i32,
    limit: i32,
) -> Result<(Vec<UserBookingResponse>, i64), sqlx::Error> {
    let offset = ((page - 1) * limit) as i64;

    let base_query = "
        SELECT b.id as booking_id, b.booking_number, p.id as provider_id, p.name as provider_name, p.avatar_url as provider_image, 
               s.name as service_type, b.scheduled_at::text as scheduled_date_time, b.status, b.address, b.estimated_cost, b.created_at::text as created_at
        FROM carecrew_bookings b
        JOIN carecrew_providers p ON b.provider_id = p.id
        JOIN carecrew_services s ON b.service_id = s.id
        WHERE b.user_id = $1";

    let count_query = "SELECT COUNT(*) as total FROM carecrew_bookings WHERE user_id = $1";

    let (rows, total): (Vec<UserBookingResponse>, i64) = if let Some(st) = status {
        let q = format!(
            "{} AND b.status = $2 ORDER BY b.created_at DESC LIMIT $3 OFFSET $4",
            base_query
        );
        let count_q = format!("{} AND status = $2", count_query);

        let t = sqlx::query(&count_q)
            .bind(user_id)
            .bind(st)
            .fetch_one(db)
            .await?
            .get("total");
        let r = sqlx::query_as::<_, UserBookingResponse>(&q)
            .bind(user_id)
            .bind(st)
            .bind(limit as i64)
            .bind(offset)
            .fetch_all(db)
            .await?;
        (r, t)
    } else {
        let q = format!(
            "{} ORDER BY b.created_at DESC LIMIT $2 OFFSET $3",
            base_query
        );
        let t = sqlx::query(count_query)
            .bind(user_id)
            .fetch_one(db)
            .await?
            .get("total");
        let r = sqlx::query_as::<_, UserBookingResponse>(&q)
            .bind(user_id)
            .bind(limit as i64)
            .bind(offset)
            .fetch_all(db)
            .await?;
        (r, t)
    };

    Ok((rows, total))
}

pub async fn get_provider_bookings_v2(
    db: &Pool<Postgres>,
    provider_id: Uuid,
    status: Option<&str>,
    page: i32,
    limit: i32,
) -> Result<(Vec<ProviderBookingResponse>, i64), sqlx::Error> {
    let offset = ((page - 1) * limit) as i64;

    let base_query = "
        SELECT b.id as booking_id, b.booking_number, u.first_name || ' ' || u.last_name as customer_name, u.phone_no as customer_phone, u.profile_picture as customer_image,
               s.name as service_type, b.scheduled_at::text as scheduled_date_time, b.status, b.address, b.problem_description, b.estimated_cost, b.created_at::text as created_at
        FROM carecrew_bookings b
        JOIN users u ON b.user_id = u.id
        JOIN carecrew_services s ON b.service_id = s.id
        WHERE b.provider_id = $1";

    let count_query = "SELECT COUNT(*) as total FROM carecrew_bookings WHERE provider_id = $1";

    let (rows, total): (Vec<ProviderBookingResponse>, i64) = if let Some(st) = status {
        let q = format!(
            "{} AND b.status = $2 ORDER BY b.created_at DESC LIMIT $3 OFFSET $4",
            base_query
        );
        let count_q = format!("{} AND status = $2", count_query);

        let t = sqlx::query(&count_q)
            .bind(provider_id)
            .bind(st)
            .fetch_one(db)
            .await?
            .get("total");
        let r = sqlx::query_as::<_, ProviderBookingResponse>(&q)
            .bind(provider_id)
            .bind(st)
            .bind(limit as i64)
            .bind(offset)
            .fetch_all(db)
            .await?;
        (r, t)
    } else {
        let q = format!(
            "{} ORDER BY b.created_at DESC LIMIT $2 OFFSET $3",
            base_query
        );
        let t = sqlx::query(count_query)
            .bind(provider_id)
            .fetch_one(db)
            .await?
            .get("total");
        let r = sqlx::query_as::<_, ProviderBookingResponse>(&q)
            .bind(provider_id)
            .bind(limit as i64)
            .bind(offset)
            .fetch_all(db)
            .await?;
        (r, t)
    };

    Ok((rows, total))
}

/// Returns (user_id, provider_id, status) for ownership validation.
pub async fn get_booking_owner_and_provider(
    db: &Pool<Postgres>,
    booking_id: Uuid,
) -> Result<Option<(Uuid, Uuid, String)>, sqlx::Error> {
    let row =
        sqlx::query("SELECT user_id, provider_id, status FROM carecrew_bookings WHERE id = $1")
            .bind(booking_id)
            .fetch_optional(db)
            .await?;

    Ok(row.map(|r| {
        (
            r.get::<Uuid, _>("user_id"),
            r.get::<Uuid, _>("provider_id"),
            r.get::<String, _>("status"),
        )
    }))
}

/// Cancel a booking: set status = 'cancelled', cancelled_at = NOW(), insert tracking row.
pub async fn cancel_booking(
    db: &Pool<Postgres>,
    booking_id: Uuid,
    cancellation_reason: Option<&str>,
) -> Result<sqlx::postgres::PgRow, sqlx::Error> {
    let mut tx = db.begin().await?;

    let row = sqlx::query(
        r#"
        UPDATE carecrew_bookings
        SET status = 'cancelled',
            cancelled_at = NOW(),
            cancellation_reason = $2,
            updated_at = NOW()
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(booking_id)
    .bind(cancellation_reason)
    .fetch_one(&mut *tx)
    .await?;

    let description = match cancellation_reason {
        Some(reason) => format!("Booking cancelled: {}", reason),
        None => "Booking cancelled".to_string(),
    };

    sqlx::query(
        r#"
        INSERT INTO carecrew_booking_tracking (booking_id, status, description)
        VALUES ($1, 'cancelled', $2)
        "#,
    )
    .bind(booking_id)
    .bind(&description)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(row)
}

pub async fn get_booking_details(
    db: &Pool<Postgres>,
    booking_id: Uuid,
) -> Result<Option<crate::models::carecrew::BookingDetailsResponse>, sqlx::Error> {
    use crate::models::carecrew::{BookingDetailsResponse, TrackingStatusDto};

    let query = "
        SELECT b.id as booking_id, b.booking_number, p.id as provider_id, p.name as provider_name, p.phone as provider_phone, p.avatar_url as provider_image, p.rating as provider_rating,
               s.name as service_type, b.scheduled_at::text as scheduled_date_time, b.status, b.address, b.problem_description, b.contact_number, b.estimated_cost, b.final_cost, b.payment_status,
               b.cancelled_at::text as cancelled_at, b.cancellation_reason,
               b.created_at::text as created_at, b.updated_at::text as updated_at
        FROM carecrew_bookings b
        JOIN carecrew_providers p ON b.provider_id = p.id
        JOIN carecrew_services s ON b.service_id = s.id
        WHERE b.id = $1";

    let row_opt = sqlx::query(query)
        .bind(booking_id)
        .fetch_optional(db)
        .await?;

    let row = match row_opt {
        Some(r) => r,
        None => return Ok(None),
    };

    let tracking_rows = sqlx::query_as::<_, TrackingStatusDto>(
        "SELECT status, created_at::text as timestamp, description FROM carecrew_booking_tracking WHERE booking_id = $1 ORDER BY created_at ASC"
    ).bind(booking_id).fetch_all(db).await?;

    Ok(Some(BookingDetailsResponse {
        booking_id: row.get("booking_id"),
        booking_number: row.get("booking_number"),
        provider_id: row.get("provider_id"),
        provider_name: row.get("provider_name"),
        provider_phone: row.try_get("provider_phone").ok(),
        provider_image: row.try_get("provider_image").ok(),
        provider_rating: row.try_get("provider_rating").unwrap_or(0.0),
        service_type: row.get("service_type"),
        scheduled_date_time: row.get("scheduled_date_time"),
        status: row.get("status"),
        address: row.try_get("address").ok(),
        problem_description: row.try_get("problem_description").ok(),
        contact_number: row.try_get("contact_number").ok(),
        estimated_cost: row.try_get("estimated_cost").ok(),
        final_cost: row.try_get("final_cost").ok(),
        payment_status: row.get("payment_status"),
        tracking_status: tracking_rows,
        cancelled_at: row.try_get("cancelled_at").ok(),
        cancellation_reason: row.try_get("cancellation_reason").ok(),
        created_at: row.get("created_at"),
        updated_at: row.try_get("updated_at").ok(),
    }))
}
