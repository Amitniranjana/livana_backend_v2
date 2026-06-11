use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::app_state::AppState;

// -----------------------------------------------------------------------------
// Task 7: Rent Trends
// -----------------------------------------------------------------------------

#[derive(Serialize)]
pub struct RentTrendItem {
    pub month: String,
    #[serde(rename = "avgRent")]
    pub avg_rent: i64,
}

pub async fn get_rent_trends(
    State(state): State<AppState>,
) -> Json<Vec<RentTrendItem>> {
    let query = "
        SELECT 
            TO_CHAR(created_at, 'YYYY-MM') as month,
            CAST(AVG(price) AS BIGINT) as avg_rent
        FROM listings_v2
        WHERE listing_type = 'Rent' AND price > 0
        GROUP BY TO_CHAR(created_at, 'YYYY-MM')
        ORDER BY month ASC
    ";

    let mut results = Vec::new();
    if let Ok(rows) = sqlx::query(query).fetch_all(&state.db).await {
        for row in rows {
            let month: String = row.get("month");
            let avg_rent: i64 = row.get("avg_rent");
            results.push(RentTrendItem { month, avg_rent });
        }
    }

    Json(results)
}

// -----------------------------------------------------------------------------
// Task 8: Engagement
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct EngagementQuery {
    pub limit: Option<i64>,
}

#[derive(Serialize)]
pub struct EngagementItem {
    #[serde(rename = "articleId")]
    pub article_id: Uuid,
    pub headline: String,
    pub views: i32,
    pub clicks: i32,
    pub shares: i32,
}

pub async fn get_engagement(
    State(state): State<AppState>,
    Query(params): Query<EngagementQuery>,
) -> Json<Vec<EngagementItem>> {
    let limit = params.limit.unwrap_or(50).clamp(1, 100);

    let query = "
        SELECT 
            id,
            headline,
            views,
            clicks,
            shares
        FROM news_items
        ORDER BY views DESC
        LIMIT $1
    ";

    let mut results = Vec::new();
    if let Ok(rows) = sqlx::query(query).bind(limit).fetch_all(&state.db).await {
        for row in rows {
            let article_id: Uuid = row.get("id");
            let headline: String = row.get("headline");
            let views: i32 = row.get("views");
            let clicks: i32 = row.get("clicks");
            let shares: i32 = row.get("shares");
            
            results.push(EngagementItem {
                article_id,
                headline,
                views,
                clicks,
                shares,
            });
        }
    }

    Json(results)
}

// -----------------------------------------------------------------------------
// Task 8: KYC Funnel
// -----------------------------------------------------------------------------

#[derive(Serialize)]
pub struct KycFunnel {
    pub total: i64,
    pub pending: i64,
    #[serde(rename = "pendingReview")]
    pub pending_review: i64,
    pub verified: i64,
    pub rejected: i64,
}

pub async fn get_kyc_funnel(
    State(state): State<AppState>,
) -> Json<KycFunnel> {
    let query = "
        SELECT
            COUNT(*) as total,
            COUNT(*) FILTER (WHERE verification_status = 'pending') as pending,
            COUNT(*) FILTER (WHERE verification_status = 'pending_review') as pending_review,
            COUNT(*) FILTER (WHERE verification_status = 'verified') as verified,
            COUNT(*) FILTER (WHERE verification_status = 'rejected') as rejected
        FROM kyc_submissions
    ";

    let mut funnel = KycFunnel {
        total: 0,
        pending: 0,
        pending_review: 0,
        verified: 0,
        rejected: 0,
    };

    if let Ok(row) = sqlx::query(query).fetch_one(&state.db).await {
        funnel.total = row.get("total");
        funnel.pending = row.get("pending");
        funnel.pending_review = row.get("pending_review");
        funnel.verified = row.get("verified");
        funnel.rejected = row.get("rejected");
    }

    Json(funnel)
}
