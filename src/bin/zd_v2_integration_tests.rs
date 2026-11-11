use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use livana_backend_v2::services::{
    zd_lifecycle_service::{self, ZdLifecycleError},
    zd_token_service::{self, TokenLedgerError},
};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&db_url).await.expect("Failed to connect to DB");

    println!("=== Running ZD v2 Foundation Integration Tests ===");

    // Test 1: Expiry exactly +30 days, fee calculations, and no stacked allocations
    test_issue_and_no_stacked_allocations(&pool).await;

    // Test 2: Ledger invariant (available + locked + redeemed <= issued) & lock
    test_lock_and_ledger_invariant(&pool).await;

    // Test 3: Unlock after day 30 expires tokens (does not return to available)
    test_unlock_after_day_30_expires(&pool).await;

    // Test 4: Append-only transactions trigger blocks UPDATE & DELETE
    test_append_only_trigger(&pool).await;

    // Test 5: Status lifecycle transition WHERE status = '' returns 409 INVALID_STATE
    test_invalid_status_transition_returns_409(&pool).await;

    println!("=== ALL 5 INTEGRATION SUITES PASSED CLEANLY! ===");
}

async fn create_test_application(pool: &PgPool, monthly_rent: f64) -> (Uuid, Uuid) {
    let user_id = Uuid::new_v4();
    let phone_suffix = &user_id.to_string().replace('-', "")[..8];
    let phone = format!("+9199{}", phone_suffix);
    let email = format!("test_{}@example.com", phone_suffix);
    let ref_code = format!("R{}", phone_suffix);

    sqlx::query(
        "INSERT INTO users (id, first_name, last_name, email, password, phone_no, user_role, referral_code)
         VALUES ($1, 'Test', 'User', $2, 'hashedpass', $3, 'tenant', $4)"
    )
    .bind(user_id)
    .bind(&email)
    .bind(&phone)
    .bind(&ref_code)
    .execute(pool)
    .await
    .expect("Insert user failed");

    let kyc_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO kyc_submissions (id, user_id, full_name, mobile_number, email_id, govt_id_type, govt_id_number, govt_id_document_url)
         VALUES ($1, $2, 'Test Tenant', $3, $4, 'pan', 'ABCDE1234F', 'https://example.com/pan.jpg')"
    )
    .bind(kyc_id)
    .bind(user_id)
    .bind(&phone)
    .bind(&email)
    .execute(pool)
    .await
    .expect("Insert KYC failed");

    let app_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO zero_deposits (id, user_id, monthly_rent, requested_deposit_amount, monthly_income, kyc_id, status)
         VALUES ($1, $2, $3, $3 * 3, 50000.0, $4, 'applied'::zero_deposit_status)"
    )
    .bind(app_id)
    .bind(user_id)
    .bind(monthly_rent)
    .bind(kyc_id)
    .execute(pool)
    .await
    .expect("Insert zero_deposit failed");

    (user_id, app_id)
}

async fn test_issue_and_no_stacked_allocations(pool: &PgPool) {
    println!("-> Testing Issue, Fixed +30 Days Expiry, and No Stacked Allocations...");

    let monthly_rent = 12_000.0;
    let (_user_id, app_id) = create_test_application(pool, monthly_rent).await;

    // 1. Issue tokens
    let issue_res = zd_token_service::issue(
        pool,
        app_id,
        monthly_rent,
        Some("ai_auto"),
        None,
        None,
        Some("Initial issue"),
    )
    .await
    .expect("Initial issue failed");

    assert_eq!(issue_res.issued_amount, 36_000.0);
    assert_eq!(issue_res.deposit_amount, 36_000.0);
    assert_eq!(issue_res.fee_amount, 14_000.0); // 12,000 + 2,000
    assert_eq!(issue_res.first_rent_amount, 12_000.0);

    // Verify expiry is exactly +30 days
    let now = Utc::now();
    let diff = issue_res.expires_at - now;
    assert!(diff.num_days() == 30 || diff.num_days() == 29, "Expiry must be 30 days");

    // 2. Issue a second time (re-run / admin override)
    let _issue_res2 = zd_token_service::issue(
        pool,
        app_id,
        monthly_rent,
        Some("admin_override"),
        None,
        None,
        Some("Override issue"),
    )
    .await
    .expect("Second issue failed");

    // Invariant: ONLY ONE active allocation per application!
    let active_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM zd_token_allocations WHERE zero_deposit_id = $1 AND status = 'active'"
    )
    .bind(app_id)
    .fetch_one(pool)
    .await
    .expect("Count active failed");

    assert_eq!(active_count, 1, "Must never stack active allocations!");

    // Verify previous allocation is revoked
    let prev_status: String = sqlx::query_scalar(
        "SELECT status::TEXT FROM zd_token_allocations WHERE id = $1"
    )
    .bind(issue_res.allocation_id)
    .fetch_one(pool)
    .await
    .expect("Get prev status failed");

    assert_eq!(prev_status, "revoked", "Previous allocation must be revoked");

    println!("   [PASSED] Issue +30 days expiry and no stacked allocations verified.");
}

async fn test_lock_and_ledger_invariant(pool: &PgPool) {
    println!("-> Testing Lock and Ledger Invariant (available + locked + redeemed <= issued)...");

    let monthly_rent = 10_000.0; // 3x = 30,000
    let (user_id, app_id) = create_test_application(pool, monthly_rent).await;

    let issue_res = zd_token_service::issue(pool, app_id, monthly_rent, None, None, None, None).await.unwrap();

    let redemption_id = Uuid::new_v4();
    // Lock 15,000
    zd_token_service::lock(pool, app_id, 15_000.0, redemption_id, Some(user_id), Some("tenant"), None)
        .await
        .expect("Lock failed");

    // Check balances
    let row = sqlx::query(
        "SELECT issued::FLOAT8, available::FLOAT8, locked::FLOAT8, redeemed::FLOAT8 FROM zd_token_allocations WHERE id = $1"
    )
    .bind(issue_res.allocation_id)
    .fetch_one(pool)
    .await
    .expect("Fetch allocation failed");

    use sqlx::Row;
    let issued: f64 = row.get("issued");
    let available: f64 = row.get("available");
    let locked: f64 = row.get("locked");
    let redeemed: f64 = row.get("redeemed");

    assert_eq!(issued, 30_000.0);
    assert_eq!(available, 15_000.0);
    assert_eq!(locked, 15_000.0);
    assert_eq!(redeemed, 0.0);
    assert!(available + locked + redeemed <= issued);

    // Attempting to lock 20,000 (more than available 15,000) must fail
    let err = zd_token_service::lock(pool, app_id, 20_000.0, redemption_id, Some(user_id), Some("tenant"), None)
        .await
        .unwrap_err();

    match err {
        TokenLedgerError::InsufficientTokens { requested, available: avail } => {
            assert_eq!(requested, 20_000.0);
            assert_eq!(avail, 15_000.0);
        }
        _ => panic!("Expected InsufficientTokens error"),
    }

    println!("   [PASSED] Lock and ledger invariant verified.");
}

async fn test_unlock_after_day_30_expires(pool: &PgPool) {
    println!("-> Testing Unlock After Day 30 Expires Tokens...");

    let monthly_rent = 10_000.0;
    let (_user_id, app_id) = create_test_application(pool, monthly_rent).await;

    let issue_res = zd_token_service::issue(pool, app_id, monthly_rent, None, None, None, None).await.unwrap();
    let redemption_id = Uuid::new_v4();
    zd_token_service::lock(pool, app_id, 10_000.0, redemption_id, None, None, None).await.unwrap();

    // Fast-forward expires_at to 1 day ago to simulate past day 30
    sqlx::query(
        "UPDATE zd_token_allocations SET expires_at = NOW() - INTERVAL '1 day' WHERE id = $1"
    )
    .bind(issue_res.allocation_id)
    .execute(pool)
    .await
    .unwrap();

    // Now unlock the redemption
    zd_token_service::unlock(pool, app_id, 10_000.0, redemption_id, None, None, None).await.unwrap();

    // Verify locked is 0, but available did NOT increase (tokens expired!)
    use sqlx::Row;
    let row = sqlx::query(
        "SELECT available::FLOAT8, locked::FLOAT8 FROM zd_token_allocations WHERE id = $1"
    )
    .bind(issue_res.allocation_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let avail: f64 = row.get("available");
    let lck: f64 = row.get("locked");
    assert_eq!(lck, 0.0);
    assert_eq!(avail, 20_000.0); // original 30k - 10k locked; unlocked 10k EXPIRED so available stayed 20k!

    // Verify 'expire' transaction was appended
    let last_op: String = sqlx::query_scalar(
        "SELECT operation::TEXT FROM zd_token_transactions WHERE allocation_id = $1 ORDER BY created_at DESC LIMIT 1"
    )
    .bind(issue_res.allocation_id)
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(last_op, "expire", "Unlocking after day 30 must record expire transaction");

    println!("   [PASSED] Unlock after day 30 expires tokens verified.");
}

async fn test_append_only_trigger(pool: &PgPool) {
    println!("-> Testing Append-Only Trigger on zd_token_transactions...");

    let monthly_rent = 10_000.0;
    let (_user_id, app_id) = create_test_application(pool, monthly_rent).await;

    let issue_res = zd_token_service::issue(pool, app_id, monthly_rent, None, None, None, None).await.unwrap();

    let txn_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM zd_token_transactions WHERE allocation_id = $1 LIMIT 1"
    )
    .bind(issue_res.allocation_id)
    .fetch_one(pool)
    .await
    .unwrap();

    // Attempting to UPDATE zd_token_transactions must fail via trigger
    let update_res = sqlx::query("UPDATE zd_token_transactions SET amount = 9999 WHERE id = $1")
        .bind(txn_id)
        .execute(pool)
        .await;

    assert!(update_res.is_err(), "UPDATE on zd_token_transactions must be prohibited");

    // Attempting to DELETE from zd_token_transactions must fail via trigger
    let delete_res = sqlx::query("DELETE FROM zd_token_transactions WHERE id = $1")
        .bind(txn_id)
        .execute(pool)
        .await;

    assert!(delete_res.is_err(), "DELETE on zd_token_transactions must be prohibited");

    println!("   [PASSED] Append-only trigger strictly enforces immutability.");
}

async fn test_invalid_status_transition_returns_409(pool: &PgPool) {
    println!("-> Testing Status Lifecycle Transitions (WHERE status = <expected>)...");

    let monthly_rent = 10_000.0;
    let (_user_id, app_id) = create_test_application(pool, monthly_rent).await;

    // Attempting to transition from 'tokens_issued' to 'redemption_in_progress' when it is actually 'applied' must return Err(InvalidState)
    let err = zd_lifecycle_service::transition_to_redemption_in_progress(pool, app_id)
        .await
        .unwrap_err();

    match err {
        ZdLifecycleError::InvalidState => {
            // Verify mapping to 409 INVALID_STATE
            let api_err: livana_backend_v2::utils::api_error::ApiError = err.into();
            use axum::response::IntoResponse;
            let resp = api_err.into_response();
            assert_eq!(resp.status(), axum::http::StatusCode::CONFLICT);
        }
        _ => panic!("Expected ZdLifecycleError::InvalidState"),
    }

    // Now do the valid transition: applied -> ai_review
    zd_lifecycle_service::transition_to_ai_review(pool, app_id)
        .await
        .expect("Valid transition applied -> ai_review must succeed");

    println!("   [PASSED] Status lifecycle SQL-enforcement returns 409 INVALID_STATE.");
}
