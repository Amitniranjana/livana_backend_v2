/// Analytics Repository
/// Raw SQL queries for rent trend data from the `properties` table.

use sqlx::{Pool, Postgres, Row};

/// A single daily aggregation data point.
#[derive(Debug)]
pub struct RentDataPoint {
    pub date: String,
    pub avg_rent: f64,
    pub listing_count: i64,
}

/// Fetch daily average rent grouped by date for the given filters.
///
/// Filters:
///   - `city` (required, case-insensitive match)
///   - `locality` (optional, case-insensitive ILIKE)
///   - `property_type` (optional, case-insensitive match)
///   - `days` — look-back window from NOW()
///
/// Only considers active listings with a non-null, positive price.
pub async fn get_rent_trend_data(
    db: &Pool<Postgres>,
    city: &str,
    locality: Option<&str>,
    property_type: Option<&str>,
    days: i32,
) -> Result<Vec<RentDataPoint>, sqlx::Error> {
    // Build the query dynamically based on optional filters
    let mut query = String::from(
        r#"
        SELECT DATE(created_at) AS d,
               AVG(price)::FLOAT8 AS avg_rent,
               COUNT(*)           AS listing_count
        FROM properties
        WHERE city ILIKE $1
          AND status = 'active'
          AND price IS NOT NULL
          AND price > 0
          AND created_at >= NOW() - ($2 || ' days')::INTERVAL
        "#,
    );

    // Track the bind index (we already have $1=city, $2=days)
    let mut bind_idx = 3;

    if locality.is_some() {
        query.push_str(&format!(" AND locality ILIKE ${}", bind_idx));
        bind_idx += 1;
    }
    if property_type.is_some() {
        query.push_str(&format!(" AND property_type ILIKE ${}", bind_idx));
        // bind_idx += 1; // not needed after last usage
    }

    query.push_str(" GROUP BY d ORDER BY d ASC");

    // Build the sqlx query and bind params in order
    let mut q = sqlx::query(&query)
        .bind(city)
        .bind(days.to_string()); // cast to TEXT for interval concat

    if let Some(loc) = locality {
        q = q.bind(format!("%{}%", loc));
    }
    if let Some(pt) = property_type {
        q = q.bind(pt);
    }

    let rows = q.fetch_all(db).await?;

    let data_points: Vec<RentDataPoint> = rows
        .iter()
        .map(|row| {
            let date: chrono::NaiveDate = row.get("d");
            RentDataPoint {
                date: date.format("%Y-%m-%d").to_string(),
                avg_rent: row.get::<f64, _>("avg_rent"),
                listing_count: row.get::<i64, _>("listing_count"),
            }
        })
        .collect();

    Ok(data_points)
}

/// Returns the overall average rent for the given filters within the look-back window.
pub async fn get_overall_average_rent(
    db: &Pool<Postgres>,
    city: &str,
    locality: Option<&str>,
    property_type: Option<&str>,
    days: i32,
) -> Result<Option<f64>, sqlx::Error> {
    let mut query = String::from(
        r#"
        SELECT AVG(price)::FLOAT8 AS avg_rent
        FROM properties
        WHERE city ILIKE $1
          AND status = 'active'
          AND price IS NOT NULL
          AND price > 0
          AND created_at >= NOW() - ($2 || ' days')::INTERVAL
        "#,
    );

    let mut bind_idx = 3;

    if locality.is_some() {
        query.push_str(&format!(" AND locality ILIKE ${}", bind_idx));
        bind_idx += 1;
    }
    if property_type.is_some() {
        query.push_str(&format!(" AND property_type ILIKE ${}", bind_idx));
    }

    let mut q = sqlx::query(&query)
        .bind(city)
        .bind(days.to_string());

    if let Some(loc) = locality {
        q = q.bind(format!("%{}%", loc));
    }
    if let Some(pt) = property_type {
        q = q.bind(pt);
    }

    let row = q.fetch_one(db).await?;
    let avg: Option<f64> = row.try_get("avg_rent").ok();

    Ok(avg)
}
