/// Analytics Handler
/// HTTP handler for `GET /api/v1/analytics/rent-trends`.
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;

use crate::app_state::AppState;
use crate::services::analytics_service;

// ─── Query parameters ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RentTrendQuery {
    /// City name (required)
    pub city: String,
    /// Optional locality / micro-area filter
    pub locality: Option<String>,
    /// Look-back window in days (default 30)
    pub days: Option<i32>,
    /// Optional property type filter (apartment / villa / pg / flat / commercial)
    pub property_type: Option<String>,
}

// ─── Handler ──────────────────────────────────────────────────────────────────

/// GET /api/v1/analytics/rent-trends
pub async fn get_rent_trends(
    State(app_state): State<AppState>,
    Query(q): Query<RentTrendQuery>,
) -> impl IntoResponse {
    // Validate required param
    if q.city.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "message": "Query parameter 'city' is required and cannot be empty",
                "error_code": "MISSING_CITY"
            })),
        );
    }

    let days = q.days.unwrap_or(30).clamp(1, 365);

    match analytics_service::get_rent_trends(
        &app_state.db,
        q.city.trim(),
        q.locality.as_deref(),
        q.property_type.as_deref(),
        days,
    )
    .await
    {
        Ok(data) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Rent trends retrieved successfully",
                "data": data
            })),
        ),
        Err(e) => {
            log::error!("get_rent_trends DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Failed to retrieve rent trends",
                    "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

// ─── Heatmap ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct HeatmapQuery {
    /// City name (required)
    pub city: String,
    /// Optional property type filter
    pub property_type: Option<String>,
    /// Look-back window in days (default 30)
    pub days: Option<i32>,
}

/// GET /api/v1/analytics/rent-heatmap
///
/// Returns average rent broken down by locality within a city —
/// suitable for rendering a visual heatmap.
pub async fn get_rent_heatmap(
    State(app_state): State<AppState>,
    Query(q): Query<HeatmapQuery>,
) -> impl IntoResponse {
    if q.city.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "message": "Query parameter 'city' is required and cannot be empty",
                "error_code": "MISSING_CITY"
            })),
        );
    }

    let days = q.days.unwrap_or(30).clamp(1, 365);

    match analytics_service::get_rent_heatmap(
        &app_state.db,
        q.city.trim(),
        q.property_type.as_deref(),
        days,
    )
    .await
    {
        Ok(data) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Rent heatmap retrieved successfully",
                "data": data
            })),
        ),
        Err(e) => {
            log::error!("get_rent_heatmap DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Failed to retrieve rent heatmap",
                    "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

// ─── Area Comparison ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ComparisonQuery {
    /// Comma-separated list of cities (at least 2)
    pub cities: String,
    /// Optional property type filter
    pub property_type: Option<String>,
    /// Look-back window in days (default 30)
    pub days: Option<i32>,
}

/// GET /api/v1/analytics/rent-comparison
///
/// Compares rent statistics across multiple cities.
/// Pass cities as a comma-separated list: `?cities=Ahmedabad,Mumbai`
pub async fn get_rent_comparison(
    State(app_state): State<AppState>,
    Query(q): Query<ComparisonQuery>,
) -> impl IntoResponse {
    let city_list: Vec<&str> = q
        .cities
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if city_list.len() < 2 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "message": "At least 2 cities are required for comparison (comma-separated)",
                "error_code": "INSUFFICIENT_CITIES"
            })),
        );
    }

    if city_list.len() > 10 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "message": "Maximum of 10 cities can be compared at once",
                "error_code": "TOO_MANY_CITIES"
            })),
        );
    }

    let days = q.days.unwrap_or(30).clamp(1, 365);

    match analytics_service::get_rent_comparison(
        &app_state.db,
        &city_list,
        q.property_type.as_deref(),
        days,
    )
    .await
    {
        Ok(data) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Rent comparison retrieved successfully",
                "data": data
            })),
        ),
        Err(e) => {
            log::error!("get_rent_comparison DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Failed to retrieve rent comparison",
                    "error_code": "DB_ERROR"
                })),
            )
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #[test]
    fn test_days_clamping() {
        assert_eq!(0i32.clamp(1, 365), 1);
        assert_eq!(500i32.clamp(1, 365), 365);
        assert_eq!(30i32.clamp(1, 365), 30);
    }

    #[test]
    fn test_percentage_change_formula() {
        let oldest = 2900.0_f64;
        let latest = 2500.0_f64;
        let pct = ((latest - oldest) / oldest) * 100.0;
        let pct_rounded = (pct * 100.0).round() / 100.0;
        assert!(pct_rounded < 0.0);
        // Expected: ((2500 - 2900) / 2900) * 100 = -13.79...
        assert!((pct_rounded - (-13.79)).abs() < 0.1);
    }

    #[test]
    fn test_trend_direction() {
        let pct_up = 5.0;
        let pct_down = -3.0;
        let pct_stable = 0.0;

        assert_eq!(
            if pct_up > 0.0 {
                "up"
            } else if pct_up < 0.0 {
                "down"
            } else {
                "stable"
            },
            "up"
        );
        assert_eq!(
            if pct_down > 0.0 {
                "up"
            } else if pct_down < 0.0 {
                "down"
            } else {
                "stable"
            },
            "down"
        );
        assert_eq!(
            if pct_stable > 0.0 {
                "up"
            } else if pct_stable < 0.0 {
                "down"
            } else {
                "stable"
            },
            "stable"
        );
    }

    #[test]
    fn test_division_by_zero_guard() {
        let oldest = 0.0_f64;
        let latest = 100.0_f64;
        let (pct, trend) = if oldest > 0.0 {
            let p = ((latest - oldest) / oldest) * 100.0;
            (
                p,
                if p > 0.0 {
                    "up"
                } else if p < 0.0 {
                    "down"
                } else {
                    "stable"
                },
            )
        } else {
            (0.0, "stable")
        };
        assert_eq!(pct, 0.0);
        assert_eq!(trend, "stable");
    }

    #[test]
    fn test_empty_city_validation() {
        let city = "   ";
        assert!(city.trim().is_empty());
    }
}
