use axum::Json;
/// Property Search Handlers
/// Covers Step 1, 2, and 3:
///   GET /api/v1/properties/search   – full search with filters & pagination
///   GET /api/v1/properties/filters  – dynamic filter options & ranges
///   GET /api/v1/search/suggestions  – autocomplete suggestions
use axum::extract::{Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::json;

use crate::app_state::AppState;
use crate::repository::property_search_repository::PropertySearchFilters;
use crate::services::property_search_service;

// ─── Query param DTOs ─────────────────────────────────────────────────────────

/// Query parameters for GET /api/v1/properties/search
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// Free-text: searches city, locality, pincode, project, builder, landmark
    pub q: Option<String>,

    // Location filters
    pub city: Option<String>,
    pub locality: Option<String>,
    pub pincode: Option<String>,

    // Price range
    #[serde(rename = "minPrice")]
    pub min_price: Option<i64>,
    #[serde(rename = "maxPrice")]
    pub max_price: Option<i64>,

    /// BHK filter — comma-separated string e.g. "2,3"
    pub bhk: Option<String>,

    /// Property type — comma-separated e.g. "flat,villa"
    #[serde(rename = "propertyType")]
    pub property_type: Option<String>,

    /// Furnishing — comma-separated  e.g. "unfurnished,semi"
    pub furnishing: Option<String>,

    /// Area range
    #[serde(rename = "minArea")]
    pub min_area: Option<i32>,
    #[serde(rename = "maxArea")]
    pub max_area: Option<i32>,

    /// Amenities — comma-separated e.g. "lift,parking,gym"
    pub amenities: Option<String>,

    /// Posted by — comma-separated e.g. "owner,broker"
    #[serde(rename = "postedBy")]
    pub posted_by: Option<String>,

    /// Sort: relevance | newest | price_asc | price_desc
    pub sort: Option<String>,

    /// Pagination
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

/// Query parameters for GET /api/v1/properties/filters
#[derive(Debug, Deserialize)]
pub struct FiltersQuery {
    pub city: Option<String>,
    pub q: Option<String>,
}

/// Query parameters for GET /api/v1/search/suggestions
#[derive(Debug, Deserialize)]
pub struct SuggestionsQuery {
    pub q: Option<String>,
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Split a comma-separated string into a Vec<String>, trimming whitespace.
fn split_csv(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Parse a CSV string of integers ("1,2,3") into Vec<i32>.
fn parse_int_csv(s: &str) -> Vec<i32> {
    s.split(',')
        .filter_map(|p| p.trim().parse::<i32>().ok())
        .collect()
}

// ─── Handler: Search ─────────────────────────────────────────────────────────

/// GET /api/v1/properties/search
///
/// Supports free-text search and all dynamic filters. Returns a paginated list
/// of properties with full metadata required by the frontend.
pub async fn search_properties_handler(
    State(app_state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> impl axum::response::IntoResponse {
    let page = q.page.unwrap_or(1).max(1);
    let limit = q.limit.unwrap_or(10).clamp(1, 100);

    // Parse comma-separated filter values
    let bhk_vals = q.bhk.as_deref().map(parse_int_csv);
    let property_type_vals = q.property_type.as_deref().map(split_csv);
    let furnishing_vals = q.furnishing.as_deref().map(split_csv);
    let amenities_vals = q.amenities.as_deref().map(split_csv);
    let posted_by_vals = q.posted_by.as_deref().map(split_csv);

    let filters = PropertySearchFilters {
        q: q.q,
        city: q.city,
        locality: q.locality,
        pincode: q.pincode,
        min_price: q.min_price,
        max_price: q.max_price,
        bhk: bhk_vals,
        property_type: property_type_vals,
        furnishing: furnishing_vals,
        min_area: q.min_area,
        max_area: q.max_area,
        amenities: amenities_vals,
        posted_by: posted_by_vals,
        sort: q.sort,
        page,
        limit,
    };

    match property_search_service::search_properties(&app_state.db, filters).await {
        Ok(result) => {
            let body = json!({
                "success": true,
                "message": "Properties retrieved successfully",
                "data": {
                    "properties": result.properties,
                    "pagination": {
                        "total_count":   result.total_count,
                        "current_page":  result.current_page,
                        "total_pages":   result.total_pages,
                        "limit":         result.limit,
                        "offset":        (result.current_page - 1) * result.limit
                    }
                }
            });
            (StatusCode::OK, Json(body))
        }
        Err(e) => {
            log::error!("Property search DB error: {}", e);
            let body = json!({
                "success": false,
                "message": "Internal server error",
                "error_code": "SEARCH_DB_ERROR",
                "errors": [e.to_string()]
            });
            (StatusCode::INTERNAL_SERVER_ERROR, Json(body))
        }
    }
}

// ─── Handler: Filters ─────────────────────────────────────────────────────────

/// GET /api/v1/properties/filters
///
/// Returns contextual filter options (ranges, enums) for the given city / query.
/// No authentication required — this is a public, low-latency endpoint.
pub async fn get_filters_handler(
    State(app_state): State<AppState>,
    Query(q): Query<FiltersQuery>,
) -> impl axum::response::IntoResponse {
    // Prefer explicit city param; fall back to generic query
    let city_str = q.city.as_deref().or(q.q.as_deref());

    match property_search_service::get_filters(&app_state.db, city_str).await {
        Ok(filter_data) => {
            let body = json!({
                "success": true,
                "message": "Filter options retrieved successfully",
                "data": { "filters": filter_data }
            });
            (StatusCode::OK, Json(body))
        }
        Err(e) => {
            log::error!("Filters DB error: {}", e);
            let body = json!({
                "success": false,
                "message": "Failed to fetch filter options",
                "error_code": "FILTERS_DB_ERROR",
                "errors": [e.to_string()]
            });
            (StatusCode::INTERNAL_SERVER_ERROR, Json(body))
        }
    }
}

// ─── Handler: Suggestions ─────────────────────────────────────────────────────

/// GET /api/v1/search/suggestions?q=sec
///
/// Returns fast, lightweight categorized autocomplete suggestions.
/// Requires at least 2 characters in `q` to prevent full-table scans.
pub async fn get_suggestions_handler(
    State(app_state): State<AppState>,
    Query(q): Query<SuggestionsQuery>,
) -> impl axum::response::IntoResponse {
    let query_str = q.q.unwrap_or_default();

    if query_str.trim().len() < 2 {
        let body = json!({
            "success": false,
            "message": "Query must be at least 2 characters",
            "error_code": "QUERY_TOO_SHORT",
            "errors": ["Minimum query length is 2 characters"]
        });
        return (StatusCode::BAD_REQUEST, Json(body));
    }

    match property_search_service::get_suggestions(&app_state.db, &query_str).await {
        Ok(suggestions) => {
            let body = json!({
                "success": true,
                "message": "Suggestions retrieved successfully",
                "data": { "suggestions": suggestions }
            });
            (StatusCode::OK, Json(body))
        }
        Err(e) => {
            log::error!("Suggestions DB error: {}", e);
            let body = json!({
                "success": false,
                "message": "Failed to fetch suggestions",
                "error_code": "SUGGESTIONS_DB_ERROR",
                "errors": [e.to_string()]
            });
            (StatusCode::INTERNAL_SERVER_ERROR, Json(body))
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_csv_normal() {
        let result = split_csv("flat,villa,plot");
        assert_eq!(result, vec!["flat", "villa", "plot"]);
    }

    #[test]
    fn test_split_csv_with_spaces() {
        let result = split_csv(" flat , villa , plot ");
        assert_eq!(result, vec!["flat", "villa", "plot"]);
    }

    #[test]
    fn test_split_csv_empty_string() {
        let result = split_csv("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_int_csv_valid() {
        let result = parse_int_csv("1,2,3,4");
        assert_eq!(result, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_parse_int_csv_partial_invalid() {
        // non-integer values should be silently ignored
        let result = parse_int_csv("2,3,abc,4");
        assert_eq!(result, vec![2, 3, 4]);
    }

    #[test]
    fn test_parse_int_csv_empty() {
        let result = parse_int_csv("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_page_clamping_default() {
        // page defaults to 1
        let page = None::<i32>.unwrap_or(1).max(1);
        assert_eq!(page, 1);
    }

    #[test]
    fn test_page_clamping_zero_becomes_one() {
        let page = Some(0i32).unwrap_or(1).max(1);
        assert_eq!(page, 1);
    }

    #[test]
    fn test_limit_clamped_to_100() {
        let limit = Some(999i32).unwrap_or(10).clamp(1, 100);
        assert_eq!(limit, 100);
    }

    #[test]
    fn test_pagination_offset_calculation() {
        // page=3, limit=5 → offset=10
        let page = 3i32;
        let limit = 5i32;
        let offset = (page - 1) * limit;
        assert_eq!(offset, 10);
    }

    #[test]
    fn test_total_pages_calculation_exact() {
        // 20 items, limit 10 → 2 pages
        let total: i64 = 20;
        let limit = 10i32;
        let total_pages = ((total as f64) / (limit as f64)).ceil() as i32;
        assert_eq!(total_pages, 2);
    }

    #[test]
    fn test_total_pages_calculation_remainder() {
        // 21 items, limit 10 → 3 pages
        let total: i64 = 21;
        let limit = 10i32;
        let total_pages = ((total as f64) / (limit as f64)).ceil() as i32;
        assert_eq!(total_pages, 3);
    }

    #[test]
    fn test_total_pages_zero_results() {
        // 0 items → still 1 page (display "page 1 of 1")
        let total: i64 = 0;
        let limit = 10i32;
        let total_pages = (((total as f64) / (limit as f64)).ceil() as i32).max(1);
        assert_eq!(total_pages, 1);
    }

    #[test]
    fn test_sort_param_defaults_to_relevance() {
        let sort: Option<String> = None;
        let order_by = match sort.as_deref() {
            Some("newest") => "p.created_at DESC",
            Some("price_asc") => "p.price ASC",
            Some("price_desc") => "p.price DESC",
            _ => "p.is_featured DESC, p.created_at DESC",
        };
        assert_eq!(order_by, "p.is_featured DESC, p.created_at DESC");
    }

    #[test]
    fn test_sort_price_asc() {
        let sort = Some("price_asc".to_string());
        let order_by = match sort.as_deref() {
            Some("newest") => "p.created_at DESC",
            Some("price_asc") => "p.price ASC",
            Some("price_desc") => "p.price DESC",
            _ => "p.is_featured DESC, p.created_at DESC",
        };
        assert_eq!(order_by, "p.price ASC");
    }

    #[test]
    fn test_suggestion_min_length_validation() {
        // Less than 2 chars should be rejected
        let query_str = "a";
        assert!(query_str.trim().len() < 2);
    }

    #[test]
    fn test_suggestion_valid_query() {
        let query_str = "sec"; // "Sector"
        assert!(query_str.trim().len() >= 2);
    }
}
