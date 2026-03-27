// src/handlers/property_filter.rs
//
// Module 7: Property Filter API
//   7.1  GET /api/v1/properties  — Paginated, filtered property listing

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::{
        property_filter::{FilteredPropertyDto, PaginatedPropertiesData, PropertyFilterParams},
        response::ApiResponse,
    },
    utils::{api_error::ApiError, auth_extractor::AuthenticationUser},
};

// ---------------------------------------------------------------------------
// 7.1  GET /api/v1/properties — Filter properties with pagination
// ---------------------------------------------------------------------------

pub async fn filter_properties(
    State(app_state): State<AppState>,
    _auth: AuthenticationUser,
    Query(params): Query<PropertyFilterParams>,
) -> Result<impl IntoResponse, ApiError> {
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(10).min(50).max(1);
    let offset = (page - 1) * limit;

    // ── Build dynamic WHERE clause ──────────────────────────────────────
    let mut conditions = vec!["p.status = 'active'".to_string()];
    let mut next_bind = 1usize;

    enum BindVal {
        Str(String),
        I64(i64),
    }
    let mut binds: Vec<BindVal> = vec![];

    // type filter: map residential/commercial/acres → property_type values
    if let Some(ref t) = params.property_type {
        let lower = t.to_lowercase();
        let mapped = match lower.as_str() {
            "residential" => "flat",
            "commercial" => "commercial",
            "acres" => "plot",
            _ => lower.as_str(),
        };
        next_bind += 1;
        conditions.push(format!("p.property_type = ${}", next_bind - 1));
        binds.push(BindVal::Str(mapped.to_string()));
    }

    if let Some(min) = params.min_price {
        next_bind += 1;
        conditions.push(format!("p.price >= ${}", next_bind - 1));
        binds.push(BindVal::I64(min));
    }

    if let Some(max) = params.max_price {
        next_bind += 1;
        conditions.push(format!("p.price <= ${}", next_bind - 1));
        binds.push(BindVal::I64(max));
    }

    if let Some(ref city) = params.city {
        next_bind += 1;
        conditions.push(format!("p.city ILIKE ${}", next_bind - 1));
        binds.push(BindVal::Str(format!("%{}%", city)));
    }

    let where_clause = conditions.join(" AND ");

    // ── Count query ─────────────────────────────────────────────────────
    let count_sql = format!("SELECT COUNT(*) FROM properties p WHERE {}", where_clause);

    let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
    for b in &binds {
        count_q = match b {
            BindVal::Str(s) => count_q.bind(s.clone()),
            BindVal::I64(n) => count_q.bind(*n),
        };
    }

    let total_count: i64 = count_q
        .fetch_one(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let total_pages = if total_count == 0 {
        1
    } else {
        (total_count + limit - 1) / limit
    };

    // ── Data query ──────────────────────────────────────────────────────
    let limit_pos = next_bind;
    let offset_pos = next_bind + 1;

    let data_sql = format!(
        r#"
        SELECT p.id, p.title, p.price, p.city, p.property_type, p.created_at
        FROM properties p
        WHERE {}
        ORDER BY p.created_at DESC
        LIMIT ${} OFFSET ${}
        "#,
        where_clause, limit_pos, offset_pos
    );

    let mut data_q = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            Option<i64>,
            String,
            String,
            chrono::DateTime<chrono::Utc>,
        ),
    >(&data_sql);

    for b in &binds {
        data_q = match b {
            BindVal::Str(s) => data_q.bind(s.clone()),
            BindVal::I64(n) => data_q.bind(*n),
        };
    }
    data_q = data_q.bind(limit).bind(offset);

    let rows = data_q
        .fetch_all(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let properties: Vec<FilteredPropertyDto> = rows
        .into_iter()
        .map(
            |(id, title, price, city, property_type, created_at)| FilteredPropertyDto {
                id,
                title,
                price,
                location: Some(city),
                property_type,
                created_at,
            },
        )
        .collect();

    let response = ApiResponse {
        success: true,
        message: "Properties fetched successfully".to_string(),
        data: PaginatedPropertiesData {
            properties,
            total_count,
            current_page: page,
            total_pages,
        },
    };

    Ok((StatusCode::OK, Json(response)))
}
