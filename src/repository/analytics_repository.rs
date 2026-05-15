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
        WHERE city ILIKE '%' || $1 || '%'
          AND status = 'active'
          AND price IS NOT NULL
          AND price > 0
          AND created_at >= NOW() - ($2::INT * INTERVAL '1 day')
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
    let mut q = sqlx::query(&query).bind(city).bind(days);

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
    _days: i32,
) -> Result<Option<f64>, sqlx::Error> {
    let mut query = String::from(
        r#"
        SELECT AVG(price)::FLOAT8 AS avg_rent
        FROM properties
        WHERE city ILIKE '%' || $1 || '%'
          AND status = 'active'
          AND price IS NOT NULL
          AND price > 0
        "#,
    );

    let mut bind_idx = 2;

    if locality.is_some() {
        query.push_str(&format!(" AND locality ILIKE ${}", bind_idx));
        bind_idx += 1;
    }
    if property_type.is_some() {
        query.push_str(&format!(" AND property_type ILIKE ${}", bind_idx));
    }

    let mut q = sqlx::query(&query).bind(city);

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

// ────────────────────────────────────────────────────────────────────────────
//  Heatmap: avg rent per locality within a city
// ────────────────────────────────────────────────────────────────────────────

/// A single locality-level aggregation for the heatmap.
#[derive(Debug)]
pub struct HeatmapDataPoint {
    pub locality: String,
    pub avg_rent: f64,
    pub listing_count: i64,
    pub min_rent: f64,
    pub max_rent: f64,
}

/// Returns average rent per locality within a city for the given look-back window.
pub async fn get_rent_heatmap_data(
    db: &Pool<Postgres>,
    city: &str,
    property_type: Option<&str>,
    days: i32,
) -> Result<Vec<HeatmapDataPoint>, sqlx::Error> {
    let mut query = String::from(
        r#"
        SELECT COALESCE(locality, 'Unknown') AS loc,
               AVG(price)::FLOAT8             AS avg_rent,
               COUNT(*)                       AS listing_count,
               MIN(price)::FLOAT8             AS min_rent,
               MAX(price)::FLOAT8             AS max_rent
        FROM properties
        WHERE city ILIKE '%' || $1 || '%'
          AND status = 'active'
          AND price IS NOT NULL
          AND price > 0
          AND created_at >= NOW() - ($2::INT * INTERVAL '1 day')
        "#,
    );

    if property_type.is_some() {
        query.push_str(" AND property_type ILIKE $3");
    }

    query.push_str(" GROUP BY loc ORDER BY avg_rent DESC");

    let mut q = sqlx::query(&query).bind(city).bind(days);

    if let Some(pt) = property_type {
        q = q.bind(pt);
    }

    let rows = q.fetch_all(db).await?;

    let data: Vec<HeatmapDataPoint> = rows
        .iter()
        .map(|row| HeatmapDataPoint {
            locality: row.get::<String, _>("loc"),
            avg_rent: row.get::<f64, _>("avg_rent"),
            listing_count: row.get::<i64, _>("listing_count"),
            min_rent: row.get::<f64, _>("min_rent"),
            max_rent: row.get::<f64, _>("max_rent"),
        })
        .collect();

    Ok(data)
}

// ────────────────────────────────────────────────────────────────────────────
//  Area Comparison: summary stats per city
// ────────────────────────────────────────────────────────────────────────────

/// Summary stats for a single city.
#[derive(Debug)]
pub struct CitySummary {
    pub city: String,
    pub avg_rent: f64,
    pub min_rent: f64,
    pub max_rent: f64,
    pub listing_count: i64,
}

/// Returns summary rent stats for each of the supplied cities.
///
/// Builds dynamic SQL: $1 = days, $2..$N = cities (lowered), $N+1 = property_type (optional).
pub async fn get_city_rent_summary(
    db: &Pool<Postgres>,
    cities: &[&str],
    property_type: Option<&str>,
    days: i32,
) -> Result<Vec<CitySummary>, sqlx::Error> {
    // $1 = days, $2..$N = cities, $N+1 = property_type (optional)
    let city_placeholders: Vec<String> = (0..cities.len()).map(|i| format!("${}", 2 + i)).collect();
    let pt_bind_idx = 2 + cities.len();

    let mut query = format!(
        r#"
        SELECT city,
               AVG(price)::FLOAT8 AS avg_rent,
               MIN(price)::FLOAT8 AS min_rent,
               MAX(price)::FLOAT8 AS max_rent,
               COUNT(*)           AS listing_count
        FROM properties
        WHERE LOWER(city) IN ({})
          AND status = 'active'
          AND price IS NOT NULL
          AND price > 0
          AND created_at >= NOW() - ($1::INT * INTERVAL '1 day')
        "#,
        city_placeholders
            .iter()
            .map(|p| format!("LOWER({})", p))
            .collect::<Vec<_>>()
            .join(", "),
    );

    if property_type.is_some() {
        query.push_str(&format!(" AND property_type ILIKE ${}", pt_bind_idx));
    }

    query.push_str(" GROUP BY city ORDER BY avg_rent DESC");

    let mut q = sqlx::query(&query).bind(days);
    for city in cities {
        q = q.bind(city.to_lowercase());
    }
    if let Some(pt) = property_type {
        q = q.bind(pt);
    }

    let rows = q.fetch_all(db).await?;

    let summaries: Vec<CitySummary> = rows
        .iter()
        .map(|row| CitySummary {
            city: row.get::<String, _>("city"),
            avg_rent: row.get::<f64, _>("avg_rent"),
            min_rent: row.get::<f64, _>("min_rent"),
            max_rent: row.get::<f64, _>("max_rent"),
            listing_count: row.get::<i64, _>("listing_count"),
        })
        .collect();

    Ok(summaries)
}
