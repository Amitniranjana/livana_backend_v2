//! Issue 109 & ZD v2 Spec — Token Wallet Handlers
//!
//! Provides:
//! - GET /api/v1/zero-deposit/tokens/wallet
//! - GET /api/v1/zero-deposit/tokens/transactions

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    handlers::zero_deposit::require_auth,
};

#[derive(Debug, Deserialize)]
pub struct WalletTransactionsQuery {
    pub allocation_id: Option<Uuid>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct WalletAllocationItem {
    pub allocation_id: Uuid,
    pub zero_deposit_id: Uuid,
    pub property_id: Option<Uuid>,
    pub issued: f64,
    pub available: f64,
    pub locked: f64,
    pub redeemed: f64,
    pub status: String,
    pub expires_at: DateTime<Utc>,
}

pub fn mask_bank_account(acc: &str) -> String {
    let trimmed = acc.trim();
    if trimmed.len() > 4 {
        format!("****{}", &trimmed[trimmed.len() - 4..])
    } else {
        "****".to_string()
    }
}

pub async fn get_token_wallet(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_id = match require_auth(&headers, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => return e.into_response(),
    };

    let rows = sqlx::query(
        r#"SELECT 
            id as allocation_id,
            zero_deposit_id,
            property_id,
            issued::FLOAT8 as issued,
            available::FLOAT8 as available,
            locked::FLOAT8 as locked,
            redeemed::FLOAT8 as redeemed,
            status::TEXT as status,
            expires_at
           FROM zd_token_allocations
           WHERE user_id = $1
           ORDER BY created_at DESC"#
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await;

    let rows = match rows {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let now = Utc::now();
    let mut total_available: f64 = 0.0;
    let mut allocations: Vec<WalletAllocationItem> = Vec::with_capacity(rows.len());

    for row in rows {
        let allocation_id: Uuid = row.get("allocation_id");
        let zero_deposit_id: Uuid = row.get("zero_deposit_id");
        let property_id: Option<Uuid> = row.try_get("property_id").ok();
        let issued: f64 = row.get("issued");
        let available: f64 = row.get("available");
        let locked: f64 = row.get("locked");
        let redeemed: f64 = row.get("redeemed");
        let status: String = row.get("status");
        let expires_at: DateTime<Utc> = row.get("expires_at");

        if status == "active" && expires_at > now {
            total_available += available;
        }

        allocations.push(WalletAllocationItem {
            allocation_id,
            zero_deposit_id,
            property_id,
            issued,
            available,
            locked,
            redeemed,
            status,
            expires_at,
        });
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": {
                "allocations": allocations,
                "total_available": total_available
            }
        })),
    )
        .into_response()
}

pub async fn get_token_transactions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<WalletTransactionsQuery>,
) -> impl IntoResponse {
    let user_id = match require_auth(&headers, &state.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => return e.into_response(),
    };

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    // If allocation_id is specified, verify ownership
    if let Some(alloc_id) = query.allocation_id {
        let owns: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM zd_token_allocations WHERE id = $1 AND user_id = $2)"
        )
        .bind(alloc_id)
        .bind(user_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(false);

        if !owns {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "success": false,
                    "error": "NOT_OWNER",
                    "message": "Allocation does not belong to the calling user"
                })),
            )
                .into_response();
        }
    }

    let records = sqlx::query_scalar::<_, Value>(
        r#"SELECT row_to_json(t) FROM (
            SELECT 
                t.id,
                t.allocation_id,
                t.transaction_type as type,
                t.amount::FLOAT8 as amount,
                t.balance_after::FLOAT8 as balance_after,
                t.reference_id,
                t.created_at
            FROM zd_token_transactions t
            JOIN zd_token_allocations a ON t.allocation_id = a.id
            WHERE a.user_id = $1
              AND ($2::UUID IS NULL OR t.allocation_id = $2)
            ORDER BY t.created_at DESC
            LIMIT $3 OFFSET $4
        ) t"#
    )
    .bind(user_id)
    .bind(query.allocation_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await;

    match records {
        Ok(txs) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "data": { "transactions": txs }
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

