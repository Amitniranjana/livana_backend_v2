use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashMap;

use crate::app_state::AppState;

#[derive(Serialize)]
pub struct StatsResponse {
    #[serde(flatten)]
    pub counts: HashMap<String, i64>,
}

pub async fn get_stats(State(state): State<AppState>) -> Json<StatsResponse> {
    let mut counts = HashMap::new();
    let tables = vec![
        "users",
        "properties",
        "news_items",
        "kyc_submissions",
        "carecrew_bookings",
        "moderation_reports",
    ];

    for table in tables {
        let query = format!("SELECT COUNT(*) FROM {}", table);
        if let Ok(count) = sqlx::query_scalar::<_, i64>(&query)
            .fetch_one(&state.db)
            .await
        {
            // The task asks to map news_items to news, etc.
            let key = match table {
                "news_items" => "news",
                "kyc_submissions" => "kyc",
                "carecrew_bookings" => "bookings",
                "moderation_reports" => "reports",
                _ => table,
            };
            counts.insert(key.to_string(), count);
        }
    }

    Json(StatsResponse { counts })
}

#[derive(Deserialize)]
pub struct TrendQuery {
    #[serde(default = "default_days")]
    pub days: i32,
    pub entity: String,
}

fn default_days() -> i32 {
    30
}

#[derive(Serialize, Clone)]
pub struct TrendItem {
    pub date: String,
    pub count: i64,
}

pub async fn get_stats_trend(
    State(state): State<AppState>,
    Query(params): Query<TrendQuery>,
) -> Json<Vec<TrendItem>> {
    let table = match params.entity.as_str() {
        "users" => "users",
        "properties" => "properties",
        "news" => "news_items",
        "kyc" => "kyc_submissions",
        "bookings" => "carecrew_bookings",
        "reports" => "moderation_reports",
        _ => return Json(vec![]),
    };

    let query = format!(
        "SELECT DATE(created_at) as date, COUNT(*) as count 
         FROM {} 
         WHERE created_at >= NOW() - INTERVAL '{} days' 
         GROUP BY DATE(created_at) 
         ORDER BY DATE(created_at) ASC",
        table, params.days
    );

    let rows = sqlx::query(&query).fetch_all(&state.db).await;

    let mut results_map: HashMap<String, i64> = HashMap::new();
    
    if let Ok(rows) = rows {
        for row in rows {
            let date: chrono::NaiveDate = row.get("date");
            let count: i64 = row.get("count");
            results_map.insert(date.format("%Y-%m-%d").to_string(), count);
        }
    }

    // Fill missing days
    let mut trend_data = Vec::new();
    let today = chrono::Utc::now().naive_utc().date();
    for i in (0..params.days).rev() {
        let d = today - chrono::Duration::days(i as i64);
        let date_str = d.format("%Y-%m-%d").to_string();
        let count = results_map.get(&date_str).copied().unwrap_or(0);
        trend_data.push(TrendItem {
            date: date_str,
            count,
        });
    }

    Json(trend_data)
}

#[derive(Deserialize)]
pub struct LocationQuery {
    pub entity: String,
    #[serde(rename = "groupBy")]
    pub group_by: String,
}

#[derive(Serialize)]
pub struct LocationItem {
    pub location: String,
    pub count: i64,
}

pub async fn get_stats_location(
    State(state): State<AppState>,
    Query(params): Query<LocationQuery>,
) -> Json<Vec<LocationItem>> {
    let table = match params.entity.as_str() {
        "users" => "users",
        "properties" => "listings_v2", // using listings_v2 as the main properties table for location grouping as per schema
        _ => return Json(vec![]),
    };

    let column = match params.group_by.as_str() {
        "city" => "city",
        "state" => "state",
        _ => return Json(vec![]),
    };

    // Check if column exists
    let check_query = "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = $1 AND column_name = $2)";
    let exists: bool = match sqlx::query_scalar(check_query)
        .bind(table)
        .bind(column)
        .fetch_one(&state.db)
        .await
    {
        Ok(e) => e,
        Err(_) => false,
    };

    if !exists {
        return Json(vec![]);
    }

    let query = format!(
        "SELECT {} as location, COUNT(*) as count 
         FROM {} 
         WHERE {} IS NOT NULL AND {} != '' 
         GROUP BY {} 
         ORDER BY count DESC 
         LIMIT 10",
        column, table, column, column, column
    );

    let rows = sqlx::query(&query).fetch_all(&state.db).await;
    
    let mut results = Vec::new();
    if let Ok(rows) = rows {
        for row in rows {
            let location: String = row.get("location");
            let count: i64 = row.get("count");
            results.push(LocationItem { location, count });
        }
    }

    Json(results)
}
