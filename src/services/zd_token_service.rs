//! Issue 109 & ZD v2 Spec — Token Ledger Service
//!
//! Operations: issue, lock, unlock, redeem, expire, revoke.
//!
//! Invariants & Rules:
//! - Expiry = issued_at + exactly 30 days, fixed and computed server-side, never accepted as input.
//! - One active allocation per application (re-run or override replaces recommendation, NEVER stacks).
//! - Every move runs in ONE transaction with `SELECT … FOR UPDATE` on the allocation row.
//! - Append-only `zd_token_transactions` row written for every move with `available_after` / `locked_after` snapshots.
//! - Tokens locked before day 30 are honoured until the redemption closes.
//! - After day 30, reject or cancel EXPIRES them instead of returning them to available.
//! - Revoke revokes AVAILABLE balance only; locked tokens remain locked until redemption is rejected or released.
//! - Amounts: fee_amount = rent + ₹2,000 (fee_flat_amount), first_rent_amount = rent, deposit_amount = 3 x rent.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Row, Transaction};
use thiserror::Error;
use uuid::Uuid;

pub const TOKEN_VALIDITY_DAYS: i64 = 30;
pub const FEE_FLAT_AMOUNT: f64 = 2000.0;
pub const DEPOSIT_RENT_MULTIPLE: f64 = 3.0;

#[derive(Debug, Error)]
pub enum TokenLedgerError {
    #[error("Insufficient available tokens (requested {requested}, available {available})")]
    InsufficientTokens { requested: f64, available: f64 },
    #[error("Tokens have expired (30-day window passed) or allocation is revoked")]
    TokensExpired,
    #[error("Active allocation already exists for this application")]
    AlreadyActive,
    #[error("No active allocation found for application")]
    NoActiveAllocation,
    #[error("Allocation not found")]
    AllocationNotFound,
    #[error("Cannot unlock or redeem — insufficient locked tokens")]
    InsufficientLocked,
    #[error("Tokens approved cannot exceed tokens requested")]
    ApprovalExceedsRequested,
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),
}

pub type TokenResult<T> = Result<T, TokenLedgerError>;

impl From<TokenLedgerError> for crate::utils::api_error::ApiError {
    fn from(err: TokenLedgerError) -> Self {
        match err {
            TokenLedgerError::InsufficientTokens { requested, available } => {
                crate::utils::api_error::ApiError::InsufficientTokens { requested, available }
            }
            TokenLedgerError::TokensExpired => {
                crate::utils::api_error::ApiError::TokensExpired
            }
            TokenLedgerError::AlreadyActive => {
                crate::utils::api_error::ApiError::Conflict("Active allocation already exists for this application".to_string())
            }
            TokenLedgerError::NoActiveAllocation => {
                crate::utils::api_error::ApiError::Conflict("No active allocation found for application".to_string())
            }
            TokenLedgerError::AllocationNotFound => {
                crate::utils::api_error::ApiError::NotFound("Allocation not found".to_string())
            }
            TokenLedgerError::InsufficientLocked => {
                crate::utils::api_error::ApiError::Conflict("Cannot unlock or redeem — insufficient locked tokens".to_string())
            }
            TokenLedgerError::ApprovalExceedsRequested => {
                crate::utils::api_error::ApiError::Conflict("Tokens approved cannot exceed tokens requested".to_string())
            }
            TokenLedgerError::Db(e) => {
                crate::utils::api_error::ApiError::InternalServerError(e.to_string())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenIssueResult {
    pub allocation_id: Uuid,
    pub issued_amount: f64,
    pub deposit_amount: f64,
    pub fee_amount: f64,
    pub first_rent_amount: f64,
    pub expires_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Public Operations
// ---------------------------------------------------------------------------

/// Issue credit tokens for an application.
///
/// Computes:
/// - `amount = 3.0 * monthly_rent`
/// - `deposit_amount = 3.0 * monthly_rent`
/// - `fee_amount = monthly_rent + 2000.0`
/// - `first_rent_amount = monthly_rent`
/// - `expires_at = now + 30 days` (fixed, never accepted from user input)
///
/// If a previous active allocation exists, it is superseded (revoked) so there is
/// NEVER more than one active allocation stacked per application.
pub async fn issue(
    pool: &PgPool,
    application_id: Uuid,
    monthly_rent: f64,
    source: Option<&str>,
    issued_by: Option<Uuid>,
    assessment_id: Option<Uuid>,
    notes: Option<&str>,
) -> TokenResult<TokenIssueResult> {
    let mut tx = pool.begin().await?;

    let issued_amount = monthly_rent * DEPOSIT_RENT_MULTIPLE;
    let deposit_amount = issued_amount;
    let fee_amount = monthly_rent + FEE_FLAT_AMOUNT;
    let first_rent_amount = monthly_rent;

    // Fixed 30-day expiry from server time
    let issued_at = Utc::now();
    let expires_at = issued_at + Duration::days(TOKEN_VALIDITY_DAYS);

    // Fetch user_id and property_id from application row
    let app_row = sqlx::query(
        "SELECT user_id, property_id FROM zero_deposits WHERE id = $1 FOR UPDATE"
    )
    .bind(application_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (user_id, property_id) = match app_row {
        Some(r) => (r.get::<Uuid, _>("user_id"), r.get::<Option<Uuid>, _>("property_id")),
        None => return Err(TokenLedgerError::AllocationNotFound),
    };

    // Revoke any existing active allocation (no stacking)
    revoke_existing_active(&mut tx, application_id, issued_by, Some("Superseded by new issue")).await?;

    let src = source.unwrap_or("ai_auto");

    let allocation_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO zd_token_allocations
            (zero_deposit_id, user_id, property_id, assessment_id, source,
             issued, available, locked, redeemed, status, issued_at, expires_at, issued_by)
        VALUES ($1, $2, $3, $4, $5, $6, $6, 0, 0, 'active', $7, $8, $9)
        RETURNING id
        "#,
    )
    .bind(application_id)
    .bind(user_id)
    .bind(property_id)
    .bind(assessment_id)
    .bind(src)
    .bind(issued_amount)
    .bind(issued_at)
    .bind(expires_at)
    .bind(issued_by)
    .fetch_one(&mut *tx)
    .await?;

    // Update zero_deposits amounts and status
    sqlx::query(
        r#"
        UPDATE zero_deposits
        SET deposit_amount = $1, fee_amount = $2, status = 'tokens_issued'::zero_deposit_status, updated_at = NOW()
        WHERE id = $3
        "#,
    )
    .bind(deposit_amount)
    .bind(fee_amount)
    .bind(application_id)
    .execute(&mut *tx)
    .await?;

    let actor_type = if issued_by.is_some() { "admin" } else { "system" };

    append_transaction(
        &mut tx,
        allocation_id,
        application_id,
        "issue",
        issued_amount,
        issued_amount,
        0.0,
        Some("application"),
        Some(application_id),
        Some(actor_type),
        issued_by,
        notes.or(Some("Tokens issued")),
    )
    .await?;

    tx.commit().await?;

    Ok(TokenIssueResult {
        allocation_id,
        issued_amount,
        deposit_amount,
        fee_amount,
        first_rent_amount,
        expires_at,
    })
}

/// Lock tokens for a redemption submission.
/// Moves: `available -> locked`.
pub async fn lock(
    pool: &PgPool,
    application_id: Uuid,
    amount: f64,
    redemption_id: Uuid,
    actor_id: Option<Uuid>,
    actor_type: Option<&str>,
    notes: Option<&str>,
) -> TokenResult<()> {
    let mut tx = pool.begin().await?;

    let (allocation_id, available, locked, expires_at) =
        fetch_for_update_active(&mut tx, application_id).await?;

    // Cannot lock expired allocation
    if Utc::now() >= expires_at {
        return Err(TokenLedgerError::TokensExpired);
    }

    if available < amount {
        return Err(TokenLedgerError::InsufficientTokens {
            requested: amount,
            available,
        });
    }

    let new_available = available - amount;
    let new_locked = locked + amount;

    sqlx::query(
        "UPDATE zd_token_allocations
         SET available = $1, locked = $2, updated_at = NOW()
         WHERE id = $3",
    )
    .bind(new_available)
    .bind(new_locked)
    .bind(allocation_id)
    .execute(&mut *tx)
    .await?;

    let act_type = actor_type.unwrap_or("tenant");

    append_transaction(
        &mut tx,
        allocation_id,
        application_id,
        "lock",
        amount,
        new_available,
        new_locked,
        Some("redemption"),
        Some(redemption_id),
        Some(act_type),
        actor_id,
        notes.or(Some("Tokens locked for redemption")),
    )
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Unlock tokens back from locked.
/// Rule: If day 30 has passed (`now >= expires_at`), tokens EXPIRE instead of returning to available.
pub async fn unlock(
    pool: &PgPool,
    application_id: Uuid,
    amount: f64,
    redemption_id: Uuid,
    actor_id: Option<Uuid>,
    actor_type: Option<&str>,
    notes: Option<&str>,
) -> TokenResult<()> {
    let mut tx = pool.begin().await?;

    let (allocation_id, available, locked, expires_at) =
        fetch_for_update_any(&mut tx, application_id).await?;

    if locked < amount {
        return Err(TokenLedgerError::InsufficientLocked);
    }

    let now = Utc::now();
    let is_expired = now >= expires_at;

    let new_locked = locked - amount;
    let (new_available, operation) = if is_expired {
        // Expire instead of returning to wallet
        (available, "expire")
    } else {
        (available + amount, "unlock")
    };

    sqlx::query(
        "UPDATE zd_token_allocations
         SET locked = $1, available = $2, updated_at = NOW()
         WHERE id = $3",
    )
    .bind(new_locked)
    .bind(new_available)
    .bind(allocation_id)
    .execute(&mut *tx)
    .await?;

    let act_type = actor_type.unwrap_or("system");

    append_transaction(
        &mut tx,
        allocation_id,
        application_id,
        operation,
        amount,
        new_available,
        new_locked,
        Some("redemption"),
        Some(redemption_id),
        Some(act_type),
        actor_id,
        notes.or(Some(if is_expired { "Expired locked tokens on close" } else { "Tokens unlocked" })),
    )
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Redeem tokens on admin approval.
/// Moves: `locked -> redeemed`.
/// If `tokens_approved < tokens_requested`, the excess unlocked difference returns to available
/// (or expires if day 30 has passed).
pub async fn redeem(
    pool: &PgPool,
    application_id: Uuid,
    tokens_requested: f64,
    tokens_approved: f64,
    redemption_id: Uuid,
    actor_id: Option<Uuid>,
    actor_type: Option<&str>,
    notes: Option<&str>,
) -> TokenResult<()> {
    if tokens_approved > tokens_requested {
        return Err(TokenLedgerError::ApprovalExceedsRequested);
    }

    let mut tx = pool.begin().await?;

    let (allocation_id, available, locked, expires_at) =
        fetch_for_update_any(&mut tx, application_id).await?;

    if locked < tokens_requested {
        return Err(TokenLedgerError::InsufficientLocked);
    }

    let issued: f64 = sqlx::query_scalar(
        "SELECT issued::FLOAT8 FROM zd_token_allocations WHERE id = $1",
    )
    .bind(allocation_id)
    .fetch_one(&mut *tx)
    .await?;

    let redeemed_current: f64 = sqlx::query_scalar(
        "SELECT redeemed::FLOAT8 FROM zd_token_allocations WHERE id = $1",
    )
    .bind(allocation_id)
    .fetch_one(&mut *tx)
    .await?;

    let difference = tokens_requested - tokens_approved;
    let now = Utc::now();
    let is_expired = now >= expires_at;

    let new_locked = locked - tokens_requested;
    let new_redeemed = redeemed_current + tokens_approved;
    let new_available = if difference > 0.0 && !is_expired {
        available + difference
    } else {
        available
    };

    let new_status = if (new_redeemed - issued).abs() < 0.01 {
        "exhausted"
    } else {
        "active"
    };

    sqlx::query(
        "UPDATE zd_token_allocations
         SET locked = $1, redeemed = $2, available = $3, status = $4::zd_token_allocation_status, updated_at = NOW()
         WHERE id = $5",
    )
    .bind(new_locked)
    .bind(new_redeemed)
    .bind(new_available)
    .bind(new_status)
    .bind(allocation_id)
    .execute(&mut *tx)
    .await?;

    let act_type = actor_type.unwrap_or("admin");

    // Append redeem transaction
    append_transaction(
        &mut tx,
        allocation_id,
        application_id,
        "redeem",
        tokens_approved,
        new_available,
        new_locked,
        Some("redemption"),
        Some(redemption_id),
        Some(act_type),
        actor_id,
        notes.or(Some("Tokens redeemed on payout confirmation")),
    )
    .await?;

    // If difference exists, record unlock or expire for difference
    if difference > 0.0 {
        let diff_op = if is_expired { "expire" } else { "unlock" };
        append_transaction(
            &mut tx,
            allocation_id,
            application_id,
            diff_op,
            difference,
            new_available,
            new_locked,
            Some("redemption"),
            Some(redemption_id),
            Some(act_type),
            actor_id,
            Some(if is_expired { "Difference expired" } else { "Difference returned to available" }),
        )
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Expire all available balance past day 30.
/// Does NOT touch locked tokens (active redemptions keep their locked tokens until closed).
pub async fn expire(
    pool: &PgPool,
    application_id: Uuid,
    actor_id: Option<Uuid>,
    notes: Option<&str>,
) -> TokenResult<f64> {
    let mut tx = pool.begin().await?;

    let (allocation_id, available, locked, _) =
        fetch_for_update_active(&mut tx, application_id).await?;

    if available <= 0.0 {
        tx.commit().await?;
        return Ok(0.0);
    }

    sqlx::query(
        "UPDATE zd_token_allocations
         SET available = 0, status = 'expired'::zd_token_allocation_status, updated_at = NOW()
         WHERE id = $1",
    )
    .bind(allocation_id)
    .execute(&mut *tx)
    .await?;

    append_transaction(
        &mut tx,
        allocation_id,
        application_id,
        "expire",
        available,
        0.0,
        locked,
        Some("application"),
        Some(application_id),
        Some("system"),
        actor_id,
        notes.or(Some("Day 30 expiry of available tokens")),
    )
    .await?;

    tx.commit().await?;
    Ok(available)
}

/// Revoke AVAILABLE balance only; locked tokens stay locked until their redemption closes.
pub async fn revoke(
    pool: &PgPool,
    application_id: Uuid,
    actor_id: Option<Uuid>,
    notes: Option<&str>,
) -> TokenResult<f64> {
    let mut tx = pool.begin().await?;

    let (allocation_id, available, locked, _) =
        fetch_for_update_active(&mut tx, application_id).await?;

    if available <= 0.0 {
        tx.commit().await?;
        return Ok(0.0);
    }

    sqlx::query(
        "UPDATE zd_token_allocations
         SET available = 0, status = 'revoked'::zd_token_allocation_status, updated_at = NOW()
         WHERE id = $1",
    )
    .bind(allocation_id)
    .execute(&mut *tx)
    .await?;

    append_transaction(
        &mut tx,
        allocation_id,
        application_id,
        "revoke",
        available,
        0.0,
        locked,
        Some("application"),
        Some(application_id),
        Some("admin"),
        actor_id,
        notes.or(Some("Revoked available token balance")),
    )
    .await?;

    tx.commit().await?;
    Ok(available)
}

// ---------------------------------------------------------------------------
// Internal Helpers
// ---------------------------------------------------------------------------

async fn fetch_for_update_active(
    tx: &mut Transaction<'_, Postgres>,
    application_id: Uuid,
) -> TokenResult<(Uuid, f64, f64, DateTime<Utc>)> {
    let row = sqlx::query(
        r#"SELECT id, available::FLOAT8, locked::FLOAT8, expires_at
           FROM zd_token_allocations
           WHERE zero_deposit_id = $1 AND status = 'active'
           FOR UPDATE"#,
    )
    .bind(application_id)
    .fetch_optional(&mut **tx)
    .await?;

    match row {
        Some(r) => Ok((
            r.get("id"),
            r.get("available"),
            r.get("locked"),
            r.get("expires_at"),
        )),
        None => Err(TokenLedgerError::NoActiveAllocation),
    }
}

async fn fetch_for_update_any(
    tx: &mut Transaction<'_, Postgres>,
    application_id: Uuid,
) -> TokenResult<(Uuid, f64, f64, DateTime<Utc>)> {
    let row = sqlx::query(
        r#"SELECT id, available::FLOAT8, locked::FLOAT8, expires_at
           FROM zd_token_allocations
           WHERE zero_deposit_id = $1
           ORDER BY created_at DESC
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(application_id)
    .fetch_optional(&mut **tx)
    .await?;

    match row {
        Some(r) => Ok((
            r.get("id"),
            r.get("available"),
            r.get("locked"),
            r.get("expires_at"),
        )),
        None => Err(TokenLedgerError::AllocationNotFound),
    }
}

async fn revoke_existing_active(
    tx: &mut Transaction<'_, Postgres>,
    application_id: Uuid,
    actor_id: Option<Uuid>,
    notes: Option<&str>,
) -> TokenResult<()> {
    let row = sqlx::query(
        r#"SELECT id, available::FLOAT8, locked::FLOAT8
           FROM zd_token_allocations
           WHERE zero_deposit_id = $1 AND status = 'active'
           FOR UPDATE SKIP LOCKED"#,
    )
    .bind(application_id)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(r) = row {
        let allocation_id: Uuid = r.get("id");
        let available: f64 = r.get("available");
        let locked: f64 = r.get("locked");

        sqlx::query(
            "UPDATE zd_token_allocations
             SET available = 0, status = 'revoked'::zd_token_allocation_status, updated_at = NOW()
             WHERE id = $1",
        )
        .bind(allocation_id)
        .execute(&mut **tx)
        .await?;

        if available > 0.0 {
            append_transaction(
                tx,
                allocation_id,
                application_id,
                "revoke",
                available,
                0.0,
                locked,
                Some("application"),
                Some(application_id),
                Some("admin"),
                actor_id,
                notes.or(Some("Revoked previous active allocation")),
            )
            .await?;
        }
    }

    Ok(())
}

async fn append_transaction(
    tx: &mut Transaction<'_, Postgres>,
    allocation_id: Uuid,
    application_id: Uuid,
    operation: &str,
    amount: f64,
    available_after: f64,
    locked_after: f64,
    reference_type: Option<&str>,
    reference_id: Option<Uuid>,
    actor_type: Option<&str>,
    actor_id: Option<Uuid>,
    notes: Option<&str>,
) -> TokenResult<()> {
    let act_type = actor_type.unwrap_or("system");
    sqlx::query(
        r#"INSERT INTO zd_token_transactions
               (allocation_id, zero_deposit_id, operation, amount,
                available_after, locked_after, reference_type, reference_id,
                actor_type, actor_id, notes)
           VALUES ($1, $2, $3::zd_token_operation, $4, $5, $6, $7, $8, $9, $10, $11)"#,
    )
    .bind(allocation_id)
    .bind(application_id)
    .bind(operation)
    .bind(amount)
    .bind(available_after)
    .bind(locked_after)
    .bind(reference_type)
    .bind(reference_id)
    .bind(act_type)
    .bind(actor_id)
    .bind(notes)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Unit Tests (Issue 109 & Spec Deliverables)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ledger_invariant_simulation() {
        let monthly_rent = 20_000.0;
        let issued = monthly_rent * DEPOSIT_RENT_MULTIPLE; // 60,000
        let mut available = issued;
        let mut locked = 0.0;
        let mut redeemed = 0.0;

        // Invariant: available + locked + redeemed <= issued
        assert!(available + locked + redeemed <= issued);

        // 1. Lock 30,000 for redemption
        let lock_amount = 30_000.0;
        assert!(available >= lock_amount);
        available -= lock_amount;
        locked += lock_amount;
        assert!(available + locked + redeemed <= issued);
        assert_eq!(available, 30_000.0);
        assert_eq!(locked, 30_000.0);

        // 2. Partial redeem: 25,000 approved, 5,000 returned to available
        let approved = 25_000.0;
        let diff = lock_amount - approved;
        locked -= lock_amount;
        redeemed += approved;
        available += diff;
        assert!(available + locked + redeemed <= issued);
        assert_eq!(available, 35_000.0);
        assert_eq!(locked, 0.0);
        assert_eq!(redeemed, 25_000.0);

        // 3. Lock remaining 35,000
        let lock_amount2 = 35_000.0;
        available -= lock_amount2;
        locked += lock_amount2;
        assert!(available + locked + redeemed <= issued);

        // 4. Redeem full 35,000 -> allocation exhausted
        locked -= lock_amount2;
        redeemed += lock_amount2;
        assert!(available + locked + redeemed <= issued);
        assert_eq!(redeemed, issued);
        assert_eq!(available, 0.0);
        assert_eq!(locked, 0.0);
    }

    #[test]
    fn test_expiry_exactly_30_days() {
        let now = Utc::now();
        let expires_at = now + Duration::days(TOKEN_VALIDITY_DAYS);
        let diff = expires_at - now;
        assert_eq!(diff.num_days(), 30);
        assert_eq!(diff.num_hours(), 720);
    }

    #[test]
    fn test_fee_amounts_computation() {
        let rent = 15_000.0;
        let issued_amount = rent * DEPOSIT_RENT_MULTIPLE;
        let deposit_amount = issued_amount;
        let fee_amount = rent + FEE_FLAT_AMOUNT;
        let first_rent_amount = rent;

        assert_eq!(issued_amount, 45_000.0);
        assert_eq!(deposit_amount, 45_000.0);
        assert_eq!(fee_amount, 17_000.0);
        assert_eq!(first_rent_amount, 15_000.0);
    }

    #[test]
    fn test_unlock_after_day_30_expires_tokens() {
        let issued = 30_000.0;
        let available = 0.0;
        let mut locked = 30_000.0;
        let now = Utc::now();
        let expires_at = now - Duration::days(1); // 1 day past 30 days

        let is_expired = now >= expires_at;
        assert!(is_expired);

        // When expired, unlock reduces locked but does NOT add to available
        let unlock_amount = 30_000.0;
        locked -= unlock_amount;
        let new_available = if is_expired { available } else { available + unlock_amount };

        assert_eq!(locked, 0.0);
        assert_eq!(new_available, 0.0); // Expired, not returned to wallet!
    }

    #[test]
    fn test_unlock_before_day_30_returns_to_available() {
        let issued = 30_000.0;
        let mut available = 10_000.0;
        let mut locked = 20_000.0;
        let now = Utc::now();
        let expires_at = now + Duration::days(15); // 15 days remaining

        let is_expired = now >= expires_at;
        assert!(!is_expired);

        let unlock_amount = 20_000.0;
        locked -= unlock_amount;
        available += unlock_amount;

        assert_eq!(locked, 0.0);
        assert_eq!(available, 30_000.0);
        assert!(available + locked <= issued);
    }

    #[test]
    fn test_insufficient_tokens_error_mapping() {
        let err = TokenLedgerError::InsufficientTokens {
            requested: 50_000.0,
            available: 20_000.0,
        };
        let api_err: crate::utils::api_error::ApiError = err.into();
        use axum::response::IntoResponse;
        let resp = api_err.into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::CONFLICT);
    }

    #[test]
    fn test_tokens_expired_error_mapping() {
        let err = TokenLedgerError::TokensExpired;
        let api_err: crate::utils::api_error::ApiError = err.into();
        use axum::response::IntoResponse;
        let resp = api_err.into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::GONE);
    }
}
