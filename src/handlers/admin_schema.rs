use axum::{extract::State, Json};
use serde::Serialize;
use sqlx::Row;

use crate::app_state::AppState;
use crate::utils::response::ApiResponse;
use crate::utils::api_error::ApiErrorResponse;
use axum::http::StatusCode;

#[derive(Serialize)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub is_primary_key: bool,
}

#[derive(Serialize)]
pub struct TableSchema {
    pub table_name: String,
    pub columns: Vec<ColumnSchema>,
    pub live_row_count: i64,
}

pub async fn get_admin_schema(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<TableSchema>>>, (StatusCode, Json<ApiErrorResponse>)> {
    let query = r#"
        SELECT 
            c.table_name,
            c.column_name,
            c.data_type,
            c.is_nullable,
            EXISTS (
                SELECT 1 
                FROM information_schema.key_column_usage kcu
                JOIN information_schema.table_constraints tc 
                  ON kcu.constraint_name = tc.constraint_name
                WHERE kcu.table_name = c.table_name 
                  AND kcu.column_name = c.column_name 
                  AND tc.constraint_type = 'PRIMARY KEY'
            ) as is_primary_key
        FROM information_schema.columns c
        WHERE c.table_schema = 'public'
        ORDER BY c.table_name, c.ordinal_position
    "#;

    let rows = sqlx::query(query)
        .fetch_all(&state.db)
        .await
        .map_err(|e| {
            let err = ApiErrorResponse {
                success: false,
                message: format!("Failed to fetch schema: {}", e),
                error_code: "INTERNAL_SERVER_ERROR".to_string(),
                errors: None,
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err))
        })?;

    let mut tables: std::collections::HashMap<String, Vec<ColumnSchema>> = std::collections::HashMap::new();

    for row in rows {
        let table_name: String = row.get("table_name");
        let column_name: String = row.get("column_name");
        let data_type: String = row.get("data_type");
        let is_nullable: String = row.get("is_nullable");
        let is_primary_key: bool = row.get("is_primary_key");

        tables.entry(table_name).or_default().push(ColumnSchema {
            name: column_name,
            data_type,
            is_nullable: is_nullable == "YES",
            is_primary_key,
        });
    }

    let mut schema_result = Vec::new();

    for (table_name, columns) in tables {
        // Query live row count dynamically
        let count_query = format!("SELECT COUNT(*) FROM \"{}\"", table_name);
        
        let live_row_count: i64 = sqlx::query_scalar(&count_query)
            .fetch_one(&state.db)
            .await
            .unwrap_or(0);

        schema_result.push(TableSchema {
            table_name,
            columns,
            live_row_count,
        });
    }
    
    // Sort tables alphabetically for consistent output
    schema_result.sort_by(|a, b| a.table_name.cmp(&b.table_name));

    Ok(Json(ApiResponse::success(schema_result, Some("Schema fetched successfully".to_string()))))
}
