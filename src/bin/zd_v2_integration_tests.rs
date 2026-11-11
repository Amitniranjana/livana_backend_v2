use chrono::{NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use livana_backend_v2::models::zero_deposit::{ApplyZeroDepositRequest, DocumentsPayload};
use livana_backend_v2::services::{
    zd_eligibility_service::{
        apply_assessment_decision, persist_assessment,
        run_rules_engine_for_application, AssessmentDecision, ExtractedBankData, TenantTier,
    },
    zd_lifecycle_service::{self, ZdLifecycleError},
    zd_tier_job,
    zd_token_service::{self, TokenLedgerError},
    zero_deposit_service::{self, ConsentUpdateResult},
};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&db_url).await.expect("Failed to connect to DB");

    println!("=== Running ZD v2 Integration Tests (Foundation + Prompt 2) ===");

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

    // Test 6: Apply and Consent ownership & status transitions
    test_apply_and_consent_ownership(&pool).await;

    // Test 7: Deterministic rules engine DB execution, persistence, and single current constraint
    test_deterministic_rules_engine_db_persistence(&pool).await;

    // Test 8: Nightly tier job database updates
    test_nightly_tier_job_db(&pool).await;

    println!("=== ALL 8 INTEGRATION SUITES PASSED CLEANLY! ===");
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

async fn test_apply_and_consent_ownership(pool: &PgPool) {
    println!("-> Testing Apply and Consent Ownership & Status Transitions...");

    let user_id = Uuid::new_v4();
    let phone_suffix = &user_id.to_string().replace('-', "")[..8];
    let phone = format!("+9198{}", phone_suffix);
    let email = format!("tenant_{}@example.com", phone_suffix);
    let ref_code = format!("T{}", phone_suffix);

    sqlx::query(
        "INSERT INTO users (id, first_name, last_name, email, password, phone_no, user_role, referral_code)
         VALUES ($1, 'Amit', 'Tenant', $2, 'hashedpass', $3, 'tenant', $4)"
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
        "INSERT INTO kyc_submissions (id, user_id, full_name, mobile_number, email_id, govt_id_type, govt_id_number, govt_id_document_url, country)
         VALUES ($1, $2, 'Amit Tenant', $3, $4, 'pan', 'ABCDE1234F', 'https://example.com/pan.jpg', 'India')"
    )
    .bind(kyc_id)
    .bind(user_id)
    .bind(&phone)
    .bind(&email)
    .execute(pool)
    .await
    .expect("Insert KYC failed");

    let apply_req = ApplyZeroDepositRequest {
        property_id: None,
        kyc_id: kyc_id.to_string(),
        monthly_rent: 10_000.0,
        tenancy_months: 11,
        employment_type: "salaried".into(),
        employer_name: Some("Infosys".into()),
        current_job_start_date: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        total_experience_months: Some(24),
        business_start_date: None,
        declared_monthly_income: 32_000.0,
        variable_pay_monthly: Some(0.0),
        is_repeat_renter: false,
        occupants: None,
        documents: Some(DocumentsPayload {
            bank_statement_url: Some("https://example.com/bank.pdf".into()),
            salary_slip_urls: vec!["https://example.com/slip1.pdf".into()],
            offer_letter_url: None,
            itr_url: None,
            gst_return_urls: vec![],
            address_proof_url: Some("https://example.com/address.pdf".into()),
        }),
        document_urls: None,
        requested_deposit_amount: None,
        deposit_amount: None,
        itr_document_url: None,
        bank_statement_url: None,
        stated_employer: None,
        device_fingerprint: Some("device-123".into()),
        bank_account_hash: Some("hash123".into()),
    };

    let deposit_amount = 30_000.0;
    let fee_amount = 12_000.0;

    let app = zero_deposit_service::apply_for_zero_deposit(
        pool,
        user_id,
        None,
        kyc_id,
        &apply_req,
        deposit_amount,
        fee_amount,
        Some("device-123"),
        Some("127.0.0.1"),
    )
    .await
    .expect("Apply for zero deposit failed");

    assert_eq!(app.deposit_amount, Some(30_000.0));
    assert_eq!(app.fee_amount, Some(12_000.0));
    assert_eq!(app.monthly_rent, 10_000.0);
    assert_eq!(app.status, livana_backend_v2::models::zero_deposit::ZeroDepositStatus::Applied);

    // 1. Consent from non-owner -> NotOwner
    let intruder_id = Uuid::new_v4();
    let intruder_res = zero_deposit_service::update_consent(pool, app.id, intruder_id, true)
        .await
        .expect("update_consent query failed");
    match intruder_res {
        ConsentUpdateResult::NotOwner => {}
        _ => panic!("Expected NotOwner when user_id does not match applicant"),
    }

    // 2. Consent = true from owner -> ai_review
    let consent_res = zero_deposit_service::update_consent(pool, app.id, user_id, true)
        .await
        .expect("update_consent query failed");
    match consent_res {
        ConsentUpdateResult::Success(updated_app) => {
            assert_eq!(updated_app.status, livana_backend_v2::models::zero_deposit::ZeroDepositStatus::AiReview);
            assert!(updated_app.consent_given);
        }
        _ => panic!("Expected Success on owner consent update"),
    }

    // 3. Consent = false from owner -> blocked
    let revoke_res = zero_deposit_service::update_consent(pool, app.id, user_id, false)
        .await
        .expect("update_consent query failed");
    match revoke_res {
        ConsentUpdateResult::Success(updated_app) => {
            assert_eq!(updated_app.status, livana_backend_v2::models::zero_deposit::ZeroDepositStatus::Blocked);
            assert!(!updated_app.consent_given);
        }
        _ => panic!("Expected Success on owner consent false"),
    }

    println!("   [PASSED] Apply & Consent ownership and status transitions verified.");
}

async fn test_deterministic_rules_engine_db_persistence(pool: &PgPool) {
    println!("-> Testing Deterministic Rules Engine DB Persistence & Constraint...");

    let monthly_rent = 10_000.0;
    let (user_id, app_id) = create_test_application(pool, monthly_rent).await;

    // Attach bureau summary and employment fields to the application row
    let bureau_json = serde_json::json!({
        "score": 735,
        "active_emi_total": 4000.0,
        "card_dues_total": 1500.0,
        "write_offs": 0,
        "settlements": 0,
        "suit_filed": false,
        "dpd_12m": 0,
        "dpd_24m": 0,
        "enquiries_3m": 1
    });

    sqlx::query(
        "UPDATE zero_deposits 
         SET bureau_summary = $1, employment_type = 'salaried', monthly_income = 32000.0,
             total_experience_months = 30, current_job_start_date = '2024-06-01'
         WHERE id = $2"
    )
    .bind(&bureau_json)
    .bind(app_id)
    .execute(pool)
    .await
    .expect("Update zero_deposits bureau summary failed");

    // Insert employer verification record (auto_approve requires verified employer)
    sqlx::query(
        "INSERT INTO zd_employer_verifications (zero_deposit_id, status, official_email, attempts, is_verified)
         VALUES ($1, 'verified', 'amit@infosys.com', 1, true)"
    )
    .bind(app_id)
    .execute(pool)
    .await
    .expect("Insert employer verification failed");

    let extracted = ExtractedBankData {
        net_take_home_monthly: Some(32000.0),
        variable_pay_monthly: Some(0.0),
        variable_pay_months_seen: 0,
        avg_monthly_credits_6m: Some(33500.0),
        salary_regular: Some(true),
        bounces_3m: 0,
        avg_monthly_balance: 14500.0,
        overdraft_days_6m: 0,
        unexplained_cash_max_month: 0.0,
        loan_app_count: 0,
        emi_debits_monthly: Some(4000.0),
    };

    // Run rules engine
    let result = run_rules_engine_for_application(pool, app_id, &extracted, &[])
        .await
        .expect("Rules engine run failed");

    assert_eq!(result.decision, AssessmentDecision::AutoApprove);
    assert_eq!(result.tier, Some(TenantTier::B));
    assert_eq!(result.recommended_token_amount, 30_000.0);

    let extracted_json = serde_json::to_value(&extracted).unwrap_or(serde_json::Value::Null);

    // Persist assessment 1
    let assessment_id1 = persist_assessment(
        pool,
        app_id,
        None,
        "v1.0",
        &result,
        &extracted_json,
        &bureau_json,
        &result.admin_summary,
    )
    .await
    .expect("Persist assessment 1 failed");

    // Check assessment 1 has is_current = true
    let is_curr1: bool = sqlx::query_scalar(
        "SELECT is_current FROM zero_deposit_ai_assessments WHERE id = $1"
    )
    .bind(assessment_id1)
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(is_curr1, "Assessment 1 must be current");

    // Apply decision -> issues tokens
    apply_assessment_decision(
        pool,
        app_id,
        user_id,
        assessment_id1,
        &result,
        monthly_rent,
    )
    .await
    .expect("Apply decision failed");

    // Verify application status changed to tokens_issued
    let app_status: String = sqlx::query_scalar(
        "SELECT status::TEXT FROM zero_deposits WHERE id = $1"
    )
    .bind(app_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(app_status, "tokens_issued");

    // Verify token allocation created
    let issued_tokens: f64 = sqlx::query_scalar(
        "SELECT issued::FLOAT8 FROM zd_token_allocations WHERE zero_deposit_id = $1"
    )
    .bind(app_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(issued_tokens, 30_000.0);

    // Persist assessment 2 (e.g. re-assessment) -> verifies is_current constraint & flipping
    let assessment_id2 = persist_assessment(
        pool,
        app_id,
        None,
        "v1.1",
        &result,
        &extracted_json,
        &bureau_json,
        &result.admin_summary,
    )
    .await
    .expect("Persist assessment 2 failed");

    let is_curr1_after: bool = sqlx::query_scalar(
        "SELECT is_current FROM zero_deposit_ai_assessments WHERE id = $1"
    )
    .bind(assessment_id1)
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(!is_curr1_after, "Assessment 1 must now have is_current = false");

    let is_curr2: bool = sqlx::query_scalar(
        "SELECT is_current FROM zero_deposit_ai_assessments WHERE id = $1"
    )
    .bind(assessment_id2)
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(is_curr2, "Assessment 2 must have is_current = true");

    println!("   [PASSED] Deterministic rules engine DB persistence & single current constraint verified.");
}

async fn test_nightly_tier_job_db(pool: &PgPool) {
    println!("-> Testing Nightly Tier Job DB Execution...");

    let monthly_rent = 10_000.0;
    let (user_id, app_id) = create_test_application(pool, monthly_rent).await;

    // Insert a tenant tier at 'B' with on_time_months = 6
    sqlx::query(
        "INSERT INTO zd_tenant_tiers (id, zero_deposit_id, user_id, tier, on_time_months, source)
         VALUES ($1, $2, $3, 'B', 6, 'ai')
         ON CONFLICT (user_id) WHERE user_id IS NOT NULL DO UPDATE SET tier = 'B', on_time_months = 6, source = 'ai'"
    )
    .bind(Uuid::new_v4())
    .bind(app_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("Insert tenant tier failed");

    // Run nightly tier job
    let summary = zd_tier_job::run_nightly_tier_job(pool).await.expect("Nightly tier job failed");
    assert!(summary.upgraded_count >= 1, "Should have upgraded at least 1 record");

    // Verify upgrade to Tier A
    let updated_tier: String = sqlx::query_scalar(
        "SELECT tier::TEXT FROM zd_tenant_tiers WHERE user_id = $1"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(updated_tier, "A", "Tier should have upgraded to A after 6 months on-time rent");

    println!("   [PASSED] Nightly tier job upgraded tenant tier correctly.");
}
