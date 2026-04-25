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
