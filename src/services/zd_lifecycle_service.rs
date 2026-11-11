//! Issue 108 — ZD Status Lifecycle Service
//!
//! Every application transition uses `WHERE status = <expected_old_status>`.
//! If 0 rows are affected → returns `Err(ZdLifecycleError::InvalidState)` → HTTP 409.
//! Fee/autopay state must NOT overwrite application status.

use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ZdLifecycleError {
    #[error("Invalid state transition — application is not in the expected status")]
    InvalidState,
    #[error("Application not found")]
    NotFound,
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),
}

pub type ZdLifecycleResult<T> = Result<T, ZdLifecycleError>;

// ---------------------------------------------------------------------------
// Application transitions
// ---------------------------------------------------------------------------

/// applied → ai_review  (triggered on consent / AI job queued)
pub async fn transition_to_ai_review(
    pool: &PgPool,
    application_id: Uuid,
) -> ZdLifecycleResult<()> {
    transition(pool, application_id, "applied", "ai_review").await
}

/// ai_review → action_required  (rules engine: some signals missing)
pub async fn transition_to_action_required(
    pool: &PgPool,
    application_id: Uuid,
) -> ZdLifecycleResult<()> {
    transition(pool, application_id, "ai_review", "action_required").await
}

/// action_required → ai_review  (tenant provides more info, re-queue)
pub async fn transition_to_ai_review_from_action_required(
    pool: &PgPool,
    application_id: Uuid,
) -> ZdLifecycleResult<()> {
    transition(pool, application_id, "action_required", "ai_review").await
}

/// ai_review → ai_manual_review  (AI unavailable or rule says MANUAL_REVIEW)
pub async fn transition_to_ai_manual_review(
    pool: &PgPool,
    application_id: Uuid,
) -> ZdLifecycleResult<()> {
    transition(pool, application_id, "ai_review", "ai_manual_review").await
}

/// ai_review → approved  (auto_approve path — admin confirmation)
/// ai_manual_review → approved
pub async fn transition_to_approved(
    tx: &mut Transaction<'_, Postgres>,
    application_id: Uuid,
    from_status: &str,
) -> ZdLifecycleResult<()> {
    transition_tx(tx, application_id, from_status, "approved").await
}

/// any non-terminal → rejected
pub async fn transition_to_rejected(
    tx: &mut Transaction<'_, Postgres>,
    application_id: Uuid,
    from_status: &str,
) -> ZdLifecycleResult<()> {
    transition_tx(tx, application_id, from_status, "rejected").await
}

/// approved → tokens_issued  (token ledger issues tokens)
pub async fn transition_to_tokens_issued(
    tx: &mut Transaction<'_, Postgres>,
    application_id: Uuid,
) -> ZdLifecycleResult<()> {
    transition_tx(tx, application_id, "approved", "tokens_issued").await
}

/// tokens_issued → redemption_in_progress  (redemption submitted)
pub async fn transition_to_redemption_in_progress(
    pool: &PgPool,
    application_id: Uuid,
) -> ZdLifecycleResult<()> {
    transition(pool, application_id, "tokens_issued", "redemption_in_progress").await
}

/// redemption_in_progress → released  (all tokens redeemed + payout done)
pub async fn transition_to_released(
    tx: &mut Transaction<'_, Postgres>,
    application_id: Uuid,
) -> ZdLifecycleResult<()> {
    transition_tx(tx, application_id, "redemption_in_progress", "released").await
}

/// tokens_issued → expired  (30-day window passes with no redemption)
pub async fn transition_to_expired(
    pool: &PgPool,
    application_id: Uuid,
) -> ZdLifecycleResult<()> {
    transition(pool, application_id, "tokens_issued", "expired").await
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

async fn transition(
    pool: &PgPool,
    application_id: Uuid,
    from_status: &str,
    to_status: &str,
) -> ZdLifecycleResult<()> {
    let res = sqlx::query(
        "UPDATE zero_deposits
         SET status = $1::zero_deposit_status, updated_at = NOW()
         WHERE id = $2 AND status = $3::zero_deposit_status",
    )
    .bind(to_status)
    .bind(application_id)
    .bind(from_status)
    .execute(pool)
    .await?;

    match res.rows_affected() {
        0 => {
            // Distinguish not-found from wrong-status
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM zero_deposits WHERE id = $1)",
            )
            .bind(application_id)
            .fetch_one(pool)
            .await?;

            if exists {
                Err(ZdLifecycleError::InvalidState)
            } else {
                Err(ZdLifecycleError::NotFound)
            }
        }
        _ => Ok(()),
    }
}

async fn transition_tx(
    tx: &mut Transaction<'_, Postgres>,
    application_id: Uuid,
    from_status: &str,
    to_status: &str,
) -> ZdLifecycleResult<()> {
    let res = sqlx::query(
        "UPDATE zero_deposits
         SET status = $1::zero_deposit_status, updated_at = NOW()
         WHERE id = $2 AND status = $3::zero_deposit_status",
    )
    .bind(to_status)
    .bind(application_id)
    .bind(from_status)
    .execute(&mut **tx)
    .await?;

    if res.rows_affected() == 0 {
        Err(ZdLifecycleError::InvalidState)
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Redemption transitions
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ZdRedemptionError {
    #[error("Invalid redemption state transition")]
    InvalidState,
    #[error("Redemption not found")]
    NotFound,
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),
}

pub type ZdRedemptionResult<T> = Result<T, ZdRedemptionError>;

pub async fn redemption_transition(
    pool: &PgPool,
    redemption_id: Uuid,
    from_status: &str,
    to_status: &str,
) -> ZdRedemptionResult<()> {
    let res = sqlx::query(
        "UPDATE zd_redemptions
         SET status = $1::zd_redemption_status, updated_at = NOW()
         WHERE id = $2 AND status = $3::zd_redemption_status",
    )
    .bind(to_status)
    .bind(redemption_id)
    .bind(from_status)
    .execute(pool)
    .await?;

    if res.rows_affected() == 0 {
        Err(ZdRedemptionError::InvalidState)
    } else {
        Ok(())
    }
}

pub async fn redemption_transition_tx(
    tx: &mut Transaction<'_, Postgres>,
    redemption_id: Uuid,
    from_status: &str,
    to_status: &str,
) -> ZdRedemptionResult<()> {
    let res = sqlx::query(
        "UPDATE zd_redemptions
         SET status = $1::zd_redemption_status, updated_at = NOW()
         WHERE id = $2 AND status = $3::zd_redemption_status",
    )
    .bind(to_status)
    .bind(redemption_id)
    .bind(from_status)
    .execute(&mut **tx)
    .await?;

    if res.rows_affected() == 0 {
        Err(ZdRedemptionError::InvalidState)
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Into ApiError conversions (Issue 108 / Shared Errors -> HTTP 409, 404, 500)
// ---------------------------------------------------------------------------

impl From<ZdLifecycleError> for crate::utils::api_error::ApiError {
    fn from(err: ZdLifecycleError) -> Self {
        match err {
            ZdLifecycleError::InvalidState => {
                crate::utils::api_error::ApiError::InvalidState(
                    "Invalid state transition — application is not in the expected status".to_string(),
                )
            }
            ZdLifecycleError::NotFound => {
                crate::utils::api_error::ApiError::NotFound("Application not found".to_string())
            }
            ZdLifecycleError::Db(e) => {
                crate::utils::api_error::ApiError::InternalServerError(e.to_string())
            }
        }
    }
}

impl From<ZdRedemptionError> for crate::utils::api_error::ApiError {
    fn from(err: ZdRedemptionError) -> Self {
        match err {
            ZdRedemptionError::InvalidState => {
                crate::utils::api_error::ApiError::InvalidState(
                    "Invalid redemption state transition".to_string(),
                )
            }
            ZdRedemptionError::NotFound => {
                crate::utils::api_error::ApiError::NotFound("Redemption not found".to_string())
            }
            ZdRedemptionError::Db(e) => {
                crate::utils::api_error::ApiError::InternalServerError(e.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    #[test]
    fn test_application_invalid_state_returns_409() {
        let err = ZdLifecycleError::InvalidState;
        let api_err: crate::utils::api_error::ApiError = err.into();
        let resp = api_err.into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn test_redemption_invalid_state_returns_409() {
        let err = ZdRedemptionError::InvalidState;
        let api_err: crate::utils::api_error::ApiError = err.into();
        let resp = api_err.into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn test_not_found_returns_404() {
        let err = ZdLifecycleError::NotFound;
        let api_err: crate::utils::api_error::ApiError = err.into();
        let resp = api_err.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}



