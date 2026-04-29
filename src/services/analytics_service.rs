/// Analytics Service Layer
/// Business logic for rent-trend analytics.
/// Delegates to `analytics_repository`, computes % change and trend direction.
use crate::repository::analytics_repository as repo;
use serde_json::{Value, json};
use sqlx::{Pool, Postgres};

/// Compute rent trend analytics for the given filters.
///
/// Returns a JSON value matching the API response contract.
pub async fn get_rent_trends(
    db: &Pool<Postgres>,
    city: &str,
    locality: Option<&str>,
    property_type: Option<&str>,
    days: i32,
) -> Result<Value, sqlx::Error> {
    // 1. Fetch daily data points
    let data_points = repo::get_rent_trend_data(db, city, locality, property_type, days).await?;

    // 2. Fetch overall average
    let overall_avg = repo::get_overall_average_rent(db, city, locality, property_type, days)
        .await?
        .unwrap_or(0.0);

    // 3. Calculate percentage change and trend
    let (percentage_change, trend) = if data_points.len() >= 2 {
        let oldest = data_points.first().unwrap().avg_rent;
        let latest = data_points.last().unwrap().avg_rent;

        if oldest > 0.0 {
            let pct = ((latest - oldest) / oldest) * 100.0;
            let pct_rounded = (pct * 100.0).round() / 100.0; // 2 decimal places

            let trend = if pct_rounded > 0.0 {
                "up"
            } else if pct_rounded < 0.0 {
                "down"
            } else {
                "stable"
            };

            (pct_rounded, trend)
        } else {
            // Division by zero guard — oldest avg is 0
            (0.0, "stable")
        }
    } else {
        // Less than 2 data points — cannot compute meaningful change
        (0.0, "stable")
    };

    // 4. Build the data_points JSON array
    let points: Vec<Value> = data_points
        .iter()
        .map(|dp| {
            json!({
                "date": dp.date,
                "avg_rent": (dp.avg_rent * 100.0).round() / 100.0,
                "listing_count": dp.listing_count,
            })
        })
        .collect();

    // 5. Assemble the response
    Ok(json!({
        "average_rent": (overall_avg * 100.0).round() / 100.0,
        "percentage_change": percentage_change,
        "trend": trend,
        "currency": "INR",
        "total_listings": data_points.iter().map(|dp| dp.listing_count).sum::<i64>(),
        "data_points": points,
    }))
}

// ─── Heatmap ─────────────────────────────────────────────────────────────────

/// Returns average rent broken down by locality within a city.
///
/// Each item in the response array represents one locality with its avg, min,
/// max rent and listing count — ready for front-end heatmap rendering.
pub async fn get_rent_heatmap(
    db: &Pool<Postgres>,
    city: &str,
    property_type: Option<&str>,
    days: i32,
) -> Result<Value, sqlx::Error> {
    let data = repo::get_rent_heatmap_data(db, city, property_type, days).await?;

    let localities: Vec<Value> = data
        .iter()
        .map(|dp| {
            json!({
                "locality": dp.locality,
                "avg_rent": (dp.avg_rent * 100.0).round() / 100.0,
                "min_rent": (dp.min_rent * 100.0).round() / 100.0,
                "max_rent": (dp.max_rent * 100.0).round() / 100.0,
                "listing_count": dp.listing_count,
            })
        })
        .collect();

    let overall_avg = if !data.is_empty() {
        let total: f64 = data
            .iter()
            .map(|d| d.avg_rent * d.listing_count as f64)
            .sum();
        let count: i64 = data.iter().map(|d| d.listing_count).sum();
        if count > 0 {
            (total / count as f64 * 100.0).round() / 100.0
        } else {
            0.0
        }
    } else {
        0.0
    };

    Ok(json!({
        "city": city,
        "currency": "INR",
        "overall_avg_rent": overall_avg,
        "locality_count": localities.len(),
        "total_listings": data.iter().map(|d| d.listing_count).sum::<i64>(),
        "localities": localities,
    }))
}

// ─── Area Comparison ─────────────────────────────────────────────────────────

/// Compares rent stats across multiple cities side-by-side.
pub async fn get_rent_comparison(
    db: &Pool<Postgres>,
    cities: &[&str],
    property_type: Option<&str>,
    days: i32,
) -> Result<Value, sqlx::Error> {
    let summaries = repo::get_city_rent_summary(db, cities, property_type, days).await?;

    let city_data: Vec<Value> = summaries
        .iter()
        .map(|s| {
            json!({
                "city": s.city,
                "avg_rent": (s.avg_rent * 100.0).round() / 100.0,
                "min_rent": (s.min_rent * 100.0).round() / 100.0,
                "max_rent": (s.max_rent * 100.0).round() / 100.0,
                "listing_count": s.listing_count,
            })
        })
        .collect();

    // Compute the cheapest and most expensive city
    let cheapest = summaries
        .iter()
        .min_by(|a, b| a.avg_rent.partial_cmp(&b.avg_rent).unwrap());
    let most_expensive = summaries
        .iter()
        .max_by(|a, b| a.avg_rent.partial_cmp(&b.avg_rent).unwrap());

    // If we have exactly 2 cities, compute the difference %
    let diff_percentage = if summaries.len() == 2 {
        let a = summaries[0].avg_rent;
        let b = summaries[1].avg_rent;
        let base = a.min(b);
        if base > 0.0 {
            Some(((a.max(b) - base) / base * 100.0 * 100.0).round() / 100.0)
        } else {
            None
        }
    } else {
        None
    };

    Ok(json!({
        "currency": "INR",
        "cities_compared": cities,
        "cities": city_data,
        "cheapest": cheapest.map(|c| &c.city),
        "most_expensive": most_expensive.map(|c| &c.city),
        "difference_percentage": diff_percentage,
    }))
}
