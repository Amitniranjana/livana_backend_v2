//! Issue 109 — Token Ledger Service
//!
//! Operations: issue, lock, unlock, redeem, expire, revoke.
//!
//! Rules:
//! - Expiry = issued_at + 30 days, always computed server-side, never accepted as input.
//! - One active allocation per application (enforced by partial unique index).
//! - Every move runs in one transaction with SELECT … FOR UPDATE on the allocation row.
//! - An append-only zd_token_transactions row is written for every operation.
//! - A redemption submitted before day 30 keeps its locked tokens until it closes.
//!   After day 30, reject/cancel expires them instead of returning them to available.

use chrono::Utc;
use sqlx::{PgPool, Postgres, Row, Transaction};
use thiserror::Error;
use uuid::Uuid;

const TOKEN_VALIDITY_DAYS: i64 = 30;

#[derive(Debug, Error)]
pub enum TokenLedgerError {
    #[error("Insufficient available tokens (requested {requested}, available {available})")]
    InsufficientTokens { requested: f64, available: f64 },
    #[error("Active allocation already exists for this application")]
    AlreadyActive,
    #[error("No active allocation found for application")]
    NoActiveAllocation,
    #[error("Allocation not found")]
    AllocationNotFound,
    #[error("Cannot unlock — insufficient locked tokens")]
    InsufficientLocked,
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),
}

pub type TokenResult<T> = Result<T, TokenLedgerError>;

// ---------------------------------------------------------------------------
// Public operations
// ---------------------------------------------------------------------------

/// Issue tokens for a newly approved application.
/// If a previous allocation exists, it is superseded (status → revoked) before
/// creating the new one, so there is always at most one active allocation.
pub async fn issue(
    pool: &PgPool,
    application_id: Uuid,
    amount: f64,
    reference_id: Option<Uuid>,
    notes: Option<&str>,
) -> TokenResult<Uuid> {
    let mut tx = pool.begin().await?;

    // Revoke any existing active allocation first (re-run / override path)
    revoke_existing_active(&mut tx, application_id, Some("Superseded by new allocation")).await?;

    let issued_at = Utc::now();
    let expires_at = issued_at + chrono::Duration::days(TOKEN_VALIDITY_DAYS);

    let allocation_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO zd_token_allocations
            (zero_deposit_id, issued, available, locked, redeemed, status, issued_at, expires_at)
        VALUES ($1, $2, $2, 0, 0, 'active', $3, $4)
        RETURNING id
        "#,
    )
    .bind(application_id)
    .bind(amount)
    .bind(issued_at)
    .bind(expires_at)
    .fetch_one(&mut *tx)
    .await?;

    append_transaction(
        &mut tx,
        allocation_id,
        application_id,
        "issue",
        amount,
        reference_id,
        notes,
    )
    .await?;

    tx.commit().await?;
    Ok(allocation_id)
}

/// Lock tokens for a pending redemption.
pub async fn lock(
    pool: &PgPool,
    application_id: Uuid,
    amount: f64,
    redemption_id: Uuid,
    notes: Option<&str>,
) -> TokenResult<()> {
    let mut tx = pool.begin().await?;

    let (allocation_id, available, locked) =
        fetch_for_update_active(&mut tx, application_id).await?;

    if available < amount {
        return Err(TokenLedgerError::InsufficientTokens {
            requested: amount,
            available,
        });
    }

    sqlx::query(
        "UPDATE zd_token_allocations
         SET available = available - $1, locked = locked + $1, updated_at = NOW()
         WHERE id = $2",
    )
    .bind(amount)
    .bind(allocation_id)
    .execute(&mut *tx)
    .await?;

    let _ = locked; // suppress unused warning

    append_transaction(
        &mut tx,
        allocation_id,
        application_id,
        "lock",
        amount,
        Some(redemption_id),
        notes,
    )
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Unlock tokens back to available (redemption cancelled before day 30).
/// After day 30, tokens are expired instead of unlocked — call `expire` instead.
pub async fn unlock(
    pool: &PgPool,
    application_id: Uuid,
    amount: f64,
    redemption_id: Uuid,
    notes: Option<&str>,
) -> TokenResult<()> {
    let mut tx = pool.begin().await?;

    let (allocation_id, _available, locked) =
        fetch_for_update_active(&mut tx, application_id).await?;

    if locked < amount {
        return Err(TokenLedgerError::InsufficientLocked);
    }

    sqlx::query(
        "UPDATE zd_token_allocations
         SET locked = locked - $1, available = available + $1, updated_at = NOW()
         WHERE id = $2",
    )
    .bind(amount)
    .bind(allocation_id)
    .execute(&mut *tx)
    .await?;

    append_transaction(
        &mut tx,
        allocation_id,
        application_id,
        "unlock",
        amount,
        Some(redemption_id),
        notes,
    )
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Redeem tokens (locked → redeemed). Called when payout is confirmed.
pub async fn redeem(
    pool: &PgPool,
    application_id: Uuid,
    amount: f64,
    redemption_id: Uuid,
    notes: Option<&str>,
) -> TokenResult<()> {
    let mut tx = pool.begin().await?;

    let (allocation_id, _available, locked) =
        fetch_for_update_active(&mut tx, application_id).await?;

    if locked < amount {
        return Err(TokenLedgerError::InsufficientLocked);
    }

    // After redeeming, check if all tokens are exhausted
    let issued: f64 = sqlx::query_scalar(
        "SELECT issued FROM zd_token_allocations WHERE id = $1",
    )
    .bind(allocation_id)
    .fetch_one(&mut *tx)
    .await?;

    let new_redeemed: f64 = sqlx::query_scalar(
        "SELECT redeemed + $1 FROM zd_token_allocations WHERE id = $2",
    )
    .bind(amount)
    .bind(allocation_id)
    .fetch_one(&mut *tx)
    .await?;

    let new_status = if (new_redeemed - issued).abs() < 0.001 {
        "exhausted"
    } else {
        "active"
    };

    sqlx::query(
        "UPDATE zd_token_allocations
         SET locked = locked - $1, redeemed = redeemed + $1, status = $3::zd_token_allocation_status, updated_at = NOW()
         WHERE id = $2",
    )
    .bind(amount)
    .bind(allocation_id)
    .bind(new_status)
    .execute(&mut *tx)
    .await?;

    append_transaction(
        &mut tx,
        allocation_id,
        application_id,
        "redeem",
        amount,
        Some(redemption_id),
        notes,
    )
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Expire all available tokens (30-day window passed, no open redemption).
/// Does NOT touch locked tokens (active redemption keeps its lock).
pub async fn expire(
    pool: &PgPool,
    application_id: Uuid,
    notes: Option<&str>,
) -> TokenResult<f64> {
    let mut tx = pool.begin().await?;

    let (allocation_id, available, _locked) =
        fetch_for_update_active(&mut tx, application_id).await?;

    if available == 0.0 {
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
        None,
        notes,
    )
    .await?;

    tx.commit().await?;
    Ok(available)
}

/// Revoke all remaining tokens (admin action / fraud).
pub async fn revoke(
    pool: &PgPool,
    application_id: Uuid,
    notes: Option<&str>,
) -> TokenResult<()> {
    let mut tx = pool.begin().await?;
    revoke_existing_active(&mut tx, application_id, notes).await?;
    tx.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// SELECT … FOR UPDATE on the active allocation row, returns (id, available, locked).
async fn fetch_for_update_active(
    tx: &mut Transaction<'_, Postgres>,
    application_id: Uuid,
) -> TokenResult<(Uuid, f64, f64)> {
    let row = sqlx::query(
        r#"SELECT id, available::FLOAT8, locked::FLOAT8
           FROM zd_token_allocations
           WHERE zero_deposit_id = $1 AND status = 'active'
           FOR UPDATE"#,
    )
    .bind(application_id)
    .fetch_optional(&mut **tx)
    .await?;

    match row {
        Some(r) => Ok((r.get("id"), r.get("available"), r.get("locked"))),
        None => Err(TokenLedgerError::NoActiveAllocation),
    }
}

/// Revoke an existing active allocation within an open transaction.
/// Writes an append-only revoke transaction row.
async fn revoke_existing_active(
    tx: &mut Transaction<'_, Postgres>,
    application_id: Uuid,
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
        let total = available + locked;

        sqlx::query(
            "UPDATE zd_token_allocations
             SET available = 0, locked = 0, status = 'revoked'::zd_token_allocation_status, updated_at = NOW()
             WHERE id = $1",
        )
        .bind(allocation_id)
        .execute(&mut **tx)
        .await?;

        if total > 0.0 {
            append_transaction(
                tx,
                allocation_id,
                application_id,
                "revoke",
                total,
                None,
                notes.or(Some("Revoked allocation")),
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
    reference_id: Option<Uuid>,
    notes: Option<&str>,
) -> TokenResult<()> {
    sqlx::query(
        r#"INSERT INTO zd_token_transactions
               (allocation_id, zero_deposit_id, operation, amount, reference_id, notes)
           VALUES ($1, $2, $3::zd_token_operation, $4, $5, $6)"#,
    )
    .bind(allocation_id)
    .bind(application_id)
    .bind(operation)
    .bind(amount)
    .bind(reference_id)
    .bind(notes)
    .execute(&mut **tx)
    .await?;

    Ok(())
}
