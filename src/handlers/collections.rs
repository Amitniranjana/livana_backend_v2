/// Issue 100 — GET /api/admin/loans/defaults/prioritized
///
/// Admin-only endpoint that returns the defaulted-loan collections queue
/// sorted by priority score (desc). Score = days_overdue × outstanding_amount
/// (weighted; see `HeuristicScorer`). Includes score + inputs in each item
/// so admins can audit the ranking.
///
/// Auth:    admin JWT (injected by `admin_auth_guard` at route layer)
/// Filters: ?limit=50&offset=0  (same as Issue 67 list endpoint)

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    dtos::collections::PrioritizedDefaultsQuery,
    services::collections_service::{get_prioritized_defaults, HeuristicScorer},
};

/// GET /api/admin/loans/defaults/prioritized
pub async fn get_prioritized_defaults_handler(
    State(state): State<AppState>,
    Query(query): Query<PrioritizedDefaultsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // v1 uses the heuristic scorer; swap this for a model-based scorer later
    // by constructing a different impl of `DefaultScorer`.
    let scorer = HeuristicScorer::default();

    let items = get_prioritized_defaults(&state.db, &query, &scorer)
        .await
        .map_err(|e| {
            log::error!("[collections] get_prioritized_defaults failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": e })),
            )
        })?;

    Ok(Json(json!({
        "success": true,
        "count": items.len(),
        "scorer": "heuristic_v1",
        "scorer_formula": "days_overdue × outstanding_amount",
        "data": items
    })))
}
