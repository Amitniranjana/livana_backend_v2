/// CareCrew Repository
/// Raw sqlx::query() functions for all CareCrew CRUD operations.

use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;

// ──────────────────────────────────────────────────────────────────────────────
// Services
// ──────────────────────────────────────────────────────────────────────────────

pub async fn list_services(
    db: &Pool<Postgres>,
) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
    sqlx::query(
        "SELECT id, name, description, icon_url, category, is_active, created_at
         FROM carecrew_services
         WHERE is_active = true
         ORDER BY name ASC"
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
         WHERE id = $1"
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

pub async fn get_provider_by_id(
    db: &Pool<Postgres>,
    id: Uuid,
) -> Result<Option<sqlx::postgres::PgRow>, sqlx::Error> {
    sqlx::query(
        "SELECT id, name, bio, service_type, city, rating, review_count, is_featured, avatar_url, phone, user_id, created_at
         FROM carecrew_providers
         WHERE id = $1 AND is_active = true"
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
) -> Result<sqlx::postgres::PgRow, sqlx::Error> {
    // Parse the scheduled_at string into a timestamptz
    let scheduled_at_ts = chrono::DateTime::parse_from_rfc3339(scheduled_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|_| sqlx::Error::Protocol("Invalid scheduled_at datetime format".into()))?;

    sqlx::query(
        r#"
        INSERT INTO carecrew_bookings (id, provider_id, service_id, user_id, scheduled_at, status, notes, created_at)
        VALUES ($1, $2, $3, $4, $5, 'pending', $6, NOW())
        RETURNING id, provider_id, service_id, user_id, scheduled_at, status, notes, created_at
        "#
    )
    .bind(id)
    .bind(provider_id)
    .bind(service_id)
    .bind(user_id)
    .bind(scheduled_at_ts)
    .bind(notes)
    .fetch_one(db)
    .await
}

pub async fn get_booking_by_id(
    db: &Pool<Postgres>,
    id: Uuid,
) -> Result<Option<sqlx::postgres::PgRow>, sqlx::Error> {
    sqlx::query(
        "SELECT id, provider_id, service_id, user_id, scheduled_at, status, notes, created_at
         FROM carecrew_bookings WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(db)
    .await
}

pub async fn update_booking_status(
    db: &Pool<Postgres>,
    id: Uuid,
    new_status: &str,
) -> Result<sqlx::postgres::PgRow, sqlx::Error> {
    sqlx::query(
        "UPDATE carecrew_bookings SET status = $1, updated_at = NOW() WHERE id = $2
         RETURNING id, provider_id, service_id, user_id, scheduled_at, status, notes, created_at, updated_at"
    )
    .bind(new_status)
    .bind(id)
    .fetch_one(db)
    .await
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
        "#
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
    let row = sqlx::query(
        "SELECT COUNT(*) as total FROM carecrew_bookings WHERE provider_id = $1"
    )
    .bind(provider_id)
    .fetch_one(db)
    .await?;
    Ok(row.get::<i64, _>("total"))
}

/// Checks if provider exists and is active.
pub async fn provider_exists(
    db: &Pool<Postgres>,
    provider_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query(
        "SELECT COUNT(*) as total FROM carecrew_providers WHERE id = $1 AND is_active = true"
    )
    .bind(provider_id)
    .fetch_one(db)
    .await?;
    Ok(row.get::<i64, _>("total") > 0)
}

/// Checks if service exists and is active.
pub async fn service_exists(
    db: &Pool<Postgres>,
    service_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query(
        "SELECT COUNT(*) as total FROM carecrew_services WHERE id = $1 AND is_active = true"
    )
    .bind(service_id)
    .fetch_one(db)
    .await?;
    Ok(row.get::<i64, _>("total") > 0)
}
