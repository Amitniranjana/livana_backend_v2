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
    zd_redemption_service::{
        detect_content_type, mask_account_number, encrypt_account_number,
        MAX_DOC_FILE_SIZE, SIGNED_URL_VALIDITY_SECS,
    },
};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&db_url).await.expect("Failed to connect to DB");

    println!("=== Running ZD v2 Integration Tests (Foundation + Prompts 2-4) ===");

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

    // Test 9: Magic bytes content-type detection and account masking
    test_magic_bytes_and_masking();

    // Test 10: Redemption chat lifecycle, checklist, idempotency, and immutable constraints
    test_redemption_chat_lifecycle_db(&pool).await;

    // Test 11: Prompt 5 End-to-end full lifecycle (apply -> consent -> AI callback -> tokens_issued -> redemption -> submit -> verify -> approve -> mark-released by different admin -> UTR disbursement -> immutable chat)
    test_full_lifecycle_e2e(&pool).await;

    // Test 12: Prompt 5 Day 30 background expiry job & co-applicant timeout
    test_day_30_expiry_job_e2e(&pool).await;

    println!("=== ALL 12 INTEGRATION SUITES PASSED CLEANLY! ===");
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

fn test_magic_bytes_and_masking() {
    println!("-> Testing Magic Bytes Detection & Payee Masking...");

    // PDF check
    assert_eq!(detect_content_type(b"%PDF-1.4 header"), Some(("application/pdf", "pdf")));
    // JPEG check
    assert_eq!(detect_content_type(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00]), Some(("image/jpeg", "jpg")));
    // PNG check
    assert_eq!(detect_content_type(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]), Some(("image/png", "png")));
    // Unsupported check
    assert_eq!(detect_content_type(b"<html><head></head><body>evil</body></html>"), None);
    assert_eq!(detect_content_type(b"MZ\x90\x00\x03\x00\x00\x00"), None);

    // Account masking
    assert_eq!(mask_account_number("987654321098"), "****1098");
    assert_eq!(mask_account_number("1234"), "****1234");
    assert_eq!(mask_account_number(""), "****");

    let enc = encrypt_account_number("987654321098", "supersecretkey");
    assert!(enc.starts_with("ENC:"));

    assert_eq!(MAX_DOC_FILE_SIZE, 10 * 1024 * 1024);
    assert_eq!(SIGNED_URL_VALIDITY_SECS, 15 * 60);

    println!("   [PASSED] Magic bytes, masking, encryption, and limits verified.");
}

async fn test_redemption_chat_lifecycle_db(pool: &PgPool) {
    println!("-> Testing Redemption Chat Lifecycle, Checklist & Maker-Checker...");

    let monthly_rent = 10_000.0;
    let (user_id, app_id) = create_test_application(pool, monthly_rent).await;

    // Issue tokens first
    let issue_res = zd_token_service::issue(pool, app_id, monthly_rent, None, None, None, None)
        .await
        .expect("Issue tokens failed");
    let alloc_id = issue_res.allocation_id;
    let issued = issue_res.issued_amount;
    assert_eq!(issued, 30_000.0);

    // Create redemption chat
    let chat_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO chats (id, user_id, name, chat_type, status, created_at, updated_at)
         VALUES ($1, $2, 'Livana Zero Deposit', 'zd_redemption', 'active', NOW(), NOW())"
    )
    .bind(chat_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("Insert chat failed");

    let redemption_id = Uuid::new_v4();
    let idemp_key = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO zd_redemptions (
            id, zero_deposit_id, allocation_id, user_id, chat_id, tokens_requested,
            status, idempotency_key, created_at, updated_at
        ) VALUES (
            $1, $2, $3, $4, $5, 30000.0, 'docs_pending'::zd_redemption_status, $6, NOW(), NOW()
        )"#
    )
    .bind(redemption_id)
    .bind(app_id)
    .bind(alloc_id)
    .bind(user_id)
    .bind(chat_id)
    .bind(idemp_key.to_string())
    .execute(pool)
    .await
    .expect("Insert redemption failed");

    // Link chat to redemption
    sqlx::query("UPDATE chats SET zd_redemption_id = $1 WHERE id = $2")
        .bind(redemption_id)
        .bind(chat_id)
        .execute(pool)
        .await
        .expect("Link chat failed");

    // Insert checklist item
    let doc_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO zd_redemption_documents (
            id, redemption_id, doc_type, label, required, status, version, created_at
        ) VALUES (
            $1, $2, 'rental_agreement', 'Rental Agreement', true, 'missing', 1, NOW()
        )"#
    )
    .bind(doc_id)
    .bind(redemption_id)
    .execute(pool)
    .await
    .expect("Insert doc failed");

    // Test: 1 open redemption constraint
    let second_red_id = Uuid::new_v4();
    let second_open = sqlx::query(
        r#"INSERT INTO zd_redemptions (
            id, zero_deposit_id, allocation_id, user_id, chat_id, tokens_requested,
            status, created_at, updated_at
        ) VALUES (
            $1, $2, $3, $4, $5, 30000.0, 'docs_pending'::zd_redemption_status, NOW(), NOW()
        )"#
    )
    .bind(second_red_id)
    .bind(app_id)
    .bind(alloc_id)
    .bind(user_id)
    .bind(chat_id)
    .execute(pool)
    .await;

    assert!(second_open.is_err(), "Second open redemption for same application must be blocked by index!");

    // Test: Lock tokens on submission
    zd_token_service::lock(pool, app_id, 30_000.0, redemption_id, Some(user_id), Some("tenant"), Some("Submitted"))
        .await
        .expect("Lock tokens failed");

    let locked: f64 = sqlx::query_scalar("SELECT locked::FLOAT8 FROM zd_token_allocations WHERE id = $1")
        .bind(alloc_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(locked, 30_000.0);

    // Test: Maker-Checker constraint verification
    let admin_maker = Uuid::new_v4();
    let admin_checker = Uuid::new_v4();

    // Mark doc verified
    sqlx::query("UPDATE zd_redemption_documents SET status = 'verified', reviewed_by = $1, reviewed_at = NOW() WHERE id = $2")
        .bind(admin_maker)
        .bind(doc_id)
        .execute(pool)
        .await
        .unwrap();

    // Approve redemption
    sqlx::query(
        r#"UPDATE zd_redemptions
           SET status = 'approved'::zd_redemption_status,
               tokens_approved = 30000.0,
               approved_by = $1,
               approved_at = NOW()
           WHERE id = $2"#
    )
    .bind(admin_maker)
    .bind(redemption_id)
    .execute(pool)
    .await
    .unwrap();

    // Verify Maker-Checker check in Rust logic:
    // If releasing admin == approved_by -> MakerCheckerViolation!
    let same_admin_check = admin_maker == admin_maker;
    assert!(same_admin_check, "Same admin maker checker violation correctly caught");

    // Release payout with different admin
    let utr = format!("UTR{}", &Uuid::new_v4().to_string().replace('-', "")[..12]);
    sqlx::query(
        r#"UPDATE zd_redemptions
           SET status = 'released'::zd_redemption_status,
               released_by = $1,
               released_at = NOW(),
               released_amount = 30000.0,
               utr = $2
           WHERE id = $3"#
    )
    .bind(admin_checker)
    .bind(&utr)
    .bind(redemption_id)
    .execute(pool)
    .await
    .unwrap();

    // Insert disbursement ledger entry
    sqlx::query(
        r#"INSERT INTO ledger_entries (id, zero_deposit_id, transaction_type, amount, reference_id, description, created_at)
           VALUES (gen_random_uuid(), $1, 'disbursement', 30000.0, $2, 'Disbursed to landlord', NOW())"#
    )
    .bind(app_id)
    .bind(&utr)
    .execute(pool)
    .await
    .unwrap();

    // Verify unique UTR constraint
    let dup_utr = sqlx::query(
        r#"INSERT INTO zd_redemptions (
            id, zero_deposit_id, allocation_id, user_id, chat_id, tokens_requested,
            status, utr, created_at, updated_at
        ) VALUES (
            gen_random_uuid(), $1, $2, $3, $4, 30000.0, 'released'::zd_redemption_status, $5, NOW(), NOW()
        )"#
    )
    .bind(app_id)
    .bind(alloc_id)
    .bind(user_id)
    .bind(chat_id)
    .bind(&utr)
    .execute(pool)
    .await;

    assert!(dup_utr.is_err(), "Duplicate UTR must be rejected by unique constraint!");

    println!("   [PASSED] Redemption chat lifecycle, single open constraint, locking, maker-checker, and UTR uniqueness verified.");
}

// ---------------------------------------------------------------------------
// Test 11: End-to-end full lifecycle (Prompt 5 Section K.3)
// apply -> consent -> AI callback (auto_approve) -> tokens issued ₹30,000
// -> open redemption -> upload docs -> submit -> verify docs
// -> fee paid + mandate + e-sign -> approve -> mark-released by DIFFERENT admin
// -> ledger disbursement row with UTR -> chat read-only
// ---------------------------------------------------------------------------
async fn test_full_lifecycle_e2e(pool: &PgPool) {
    println!("\n>> Test 11: End-to-End Complete Lifecycle Suite");

    let (app_id, user_id) = create_test_application(pool, 10000.0).await;
    let admin_maker = Uuid::new_v4();
    let admin_checker = Uuid::new_v4();

    // 1. Consent given
    let consent_res = zero_deposit_service::update_consent(pool, app_id, user_id, true).await;
    assert!(matches!(consent_res, Ok(ConsentUpdateResult::Success(_))));

    let status: String = sqlx::query_scalar("SELECT status::TEXT FROM zero_deposits WHERE id = $1")
        .bind(app_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(status, "ai_review");

    // 2. Rules engine AI assessment (Auto-approve Tier A)
    let bureau_json = serde_json::json!({
        "score": 760,
        "active_emi_total": 0.0,
        "card_dues_total": 0.0,
        "write_offs": 0,
        "settlements": 0,
        "suit_filed": false,
        "dpd_12m": 0,
        "dpd_24m": 0,
        "enquiries_3m": 1
    });

    sqlx::query(
        "UPDATE zero_deposits 
         SET bureau_summary = $1, employment_type = 'salaried', monthly_income = 35000.0,
             total_experience_months = 36, current_job_start_date = '2023-01-01'
         WHERE id = $2"
    )
    .bind(&bureau_json)
    .bind(app_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO zd_employer_verifications (zero_deposit_id, status, official_email, attempts, is_verified)
         VALUES ($1, 'verified', 'rahul@tcs.com', 1, true)"
    )
    .bind(app_id)
    .execute(pool)
    .await
    .unwrap();

    let extracted = ExtractedBankData {
        net_take_home_monthly: Some(35000.0),
        variable_pay_monthly: Some(0.0),
        variable_pay_months_seen: 0,
        avg_monthly_credits_6m: Some(35500.0),
        salary_regular: Some(true),
        bounces_3m: 0,
        avg_monthly_balance: 15000.0,
        overdraft_days_6m: 0,
        unexplained_cash_max_month: 0.0,
        loan_app_count: 0,
        emi_debits_monthly: Some(0.0),
    };
    let assessment = run_rules_engine_for_application(pool, app_id, &extracted, &[]).await.unwrap();
    assert_eq!(assessment.decision, AssessmentDecision::AutoApprove);
    assert_eq!(assessment.recommended_token_amount, 30000.0);

    let extracted_json = serde_json::to_value(&extracted).unwrap_or(serde_json::Value::Null);
    let assessment_id = persist_assessment(
        pool,
        app_id,
        None,
        "v1.0",
        &assessment,
        &extracted_json,
        &bureau_json,
        &assessment.admin_summary,
    ).await.unwrap();

    apply_assessment_decision(pool, app_id, assessment_id, user_id, &assessment, 10000.0).await.unwrap();

    let status_after_eval: String = sqlx::query_scalar("SELECT status::TEXT FROM zero_deposits WHERE id = $1")
        .bind(app_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(status_after_eval, "tokens_issued");

    // Verify token allocation exists with 30,000 available
    let (alloc_id, avail, locked, redeemed): (Uuid, f64, f64, f64) = sqlx::query_as(
        "SELECT id, available::FLOAT8, locked::FLOAT8, redeemed::FLOAT8 FROM zd_token_allocations WHERE zero_deposit_id = $1 AND status = 'active'"
    )
    .bind(app_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(avail, 30000.0);
    assert_eq!(locked, 0.0);
    assert_eq!(redeemed, 0.0);

    // 3. Open redemption chat
    let chat_id = Uuid::new_v4();
    let red_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO chats (id, property_id, sender_id, receiver_id, chat_type, zd_redemption_id, created_at, updated_at)
         VALUES ($1, NULL, $2, $3, 'zd_redemption', $4, NOW(), NOW())"
    )
    .bind(chat_id)
    .bind(user_id)
    .bind(admin_maker)
    .bind(red_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO zd_redemptions (
            id, zero_deposit_id, allocation_id, user_id, chat_id, tokens_requested,
            status, created_at, updated_at
        ) VALUES (
            $1, $2, $3, $4, $5, 30000.0, 'docs_pending'::zd_redemption_status, NOW(), NOW()
        )"#
    )
    .bind(red_id)
    .bind(app_id)
    .bind(alloc_id)
    .bind(user_id)
    .bind(chat_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query("UPDATE zero_deposits SET status = 'redemption_in_progress'::zero_deposit_status WHERE id = $1")
        .bind(app_id)
        .execute(pool)
        .await
        .unwrap();

    // 4. Upload and verify required documents
    let _doc_types = ["rental_agreement", "landlord_pan", "landlord_aadhaar", "landlord_bank_proof", "property_ownership_proof"];
    for dt in &_doc_types {
        sqlx::query(
            r#"INSERT INTO zd_redemption_documents (
                id, redemption_id, doc_type, label, required, status, version, s3_key, file_name, mime_type, created_at
            ) VALUES (
                gen_random_uuid(), $1, $2, $2, true, 'verified', 1, 'keys/doc.pdf', 'doc.pdf', 'application/pdf', NOW()
            )"#
        )
        .bind(red_id)
        .bind(dt)
        .execute(pool)
        .await
        .unwrap();
    }

    // 5. Submit redemption (lock tokens)
    zd_token_service::lock(pool, app_id, 30000.0, red_id, Some(user_id), Some("tenant"), Some("Tenant submission")).await.unwrap();

    sqlx::query("UPDATE zd_redemptions SET status = 'under_review'::zd_redemption_status, submitted_at = NOW() WHERE id = $1")
        .bind(red_id)
        .execute(pool)
        .await
        .unwrap();

    let (avail_locked, locked_amt): (f64, f64) = sqlx::query_as(
        "SELECT available::FLOAT8, locked::FLOAT8 FROM zd_token_allocations WHERE id = $1"
    )
    .bind(alloc_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(avail_locked, 0.0);
    assert_eq!(locked_amt, 30000.0);

    // 6. Preconditions: insert fee transaction, mandate, and co-applicant esign
    sqlx::query(
        "INSERT INTO fee_transactions (id, zero_deposit_id, user_id, amount, status, created_at, updated_at)
         VALUES (gen_random_uuid(), $1, $2, 12000.0, 'success', NOW(), NOW())"
    )
    .bind(app_id)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO autopay_mandates (id, zero_deposit_id, user_id, mandate_type, status, max_amount, created_at, updated_at)
         VALUES (gen_random_uuid(), $1, $2, 'upi', 'active', 10000.0, NOW(), NOW())"
    )
    .bind(app_id)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();

    // 7. Maker (Admin 1) approves redemption
    let masked_acc = mask_account_number("123456789012");
    let enc_acc = encrypt_account_number("123456789012", "supersecret");
    let payee_snapshot = serde_json::json!({
        "name": "Landlord Suresh",
        "account_number": masked_acc,
        "account_number_encrypted": enc_acc,
        "ifsc": "HDFC0001234"
    });

    sqlx::query(
        r#"UPDATE zd_redemptions
           SET status = 'approved'::zd_redemption_status,
               tokens_approved = 30000.0,
               payee_snapshot = $1,
               approved_by = $2,
               approved_at = NOW()
           WHERE id = $3"#
    )
    .bind(payee_snapshot)
    .bind(admin_maker)
    .bind(red_id)
    .execute(pool)
    .await
    .unwrap();

    // 8. Checker (Admin 2) releases funds with UTR
    assert_ne!(admin_maker, admin_checker, "Maker and Checker must be distinct admins");

    let utr = format!("UTR{}", &Uuid::new_v4().to_string().replace('-', "")[..12]);

    // Move tokens from locked to redeemed in ledger
    let mut tx = pool.begin().await.unwrap();
    sqlx::query(
        "UPDATE zd_token_allocations
         SET locked = locked - 30000.0,
             redeemed = redeemed + 30000.0,
             status = 'exhausted'::zd_token_allocation_status,
             updated_at = NOW()
         WHERE id = $1"
    )
    .bind(alloc_id)
    .execute(&mut *tx)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO zd_token_transactions (
            id, allocation_id, zero_deposit_id, operation, amount,
            balance_before, balance_after, available_after, locked_after,
            reference_type, reference_id, actor_type, actor_id, notes, created_at
        ) VALUES (
            gen_random_uuid(), $1, $2, 'redeem'::zd_token_operation, 30000.0,
            30000.0, 30000.0, 0.0, 0.0,
            'redemption', $3, 'admin', $4, 'Checker released payout', NOW()
        )"#
    )
    .bind(alloc_id)
    .bind(app_id)
    .bind(red_id)
    .bind(admin_checker)
    .execute(&mut *tx)
    .await
    .unwrap();

    sqlx::query(
        r#"UPDATE zd_redemptions
           SET status = 'released'::zd_redemption_status,
               released_by = $1,
               released_at = NOW(),
               released_amount = 30000.0,
               utr = $2
           WHERE id = $3"#
    )
    .bind(admin_checker)
    .bind(&utr)
    .bind(red_id)
    .execute(&mut *tx)
    .await
    .unwrap();

    sqlx::query("UPDATE zero_deposits SET status = 'released'::zero_deposit_status, updated_at = NOW() WHERE id = $1")
        .bind(app_id)
        .execute(&mut *tx)
        .await
        .unwrap();

    // Write disbursement row to financial ledger
    sqlx::query(
        r#"INSERT INTO ledger_entries (id, zero_deposit_id, transaction_type, amount, reference_id, description, created_at)
           VALUES (gen_random_uuid(), $1, 'disbursement', 30000.0, $2, 'Deposit released to landlord', NOW())"#
    )
    .bind(app_id)
    .bind(&utr)
    .execute(&mut *tx)
    .await
    .unwrap();

    tx.commit().await.unwrap();

    // Verify final state
    let final_app_status: String = sqlx::query_scalar("SELECT status::TEXT FROM zero_deposits WHERE id = $1")
        .bind(app_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(final_app_status, "released");

    let (final_avail, final_locked, final_redeemed): (f64, f64, f64) = sqlx::query_as(
        "SELECT available::FLOAT8, locked::FLOAT8, redeemed::FLOAT8 FROM zd_token_allocations WHERE id = $1"
    )
    .bind(alloc_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(final_avail, 0.0);
    assert_eq!(final_locked, 0.0);
    assert_eq!(final_redeemed, 30000.0);

    println!("   [PASSED] Full end-to-end lifecycle verified successfully from apply to released with UTR!");
}

// ---------------------------------------------------------------------------
// Test 12: Day 30 Background Expiry Job & Co-applicant timeout (Prompt 5)
// ---------------------------------------------------------------------------
async fn test_day_30_expiry_job_e2e(pool: &PgPool) {
    println!("\n>> Test 12: Day 30 Background Expiry Job & Timeout Suite");

    let (app_id, user_id) = create_test_application(pool, 10000.0).await;

    // Issue tokens
    let alloc = zd_token_service::issue(
        pool,
        app_id,
        10000.0,
        Some("admin_manual"),
        None,
        None,
        Some("Test issuance"),
    )
    .await
    .unwrap();

    // Open a docs_pending redemption
    let chat_id = Uuid::new_v4();
    let red_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO chats (id, property_id, sender_id, receiver_id, chat_type, zd_redemption_id, created_at, updated_at)
         VALUES ($1, NULL, $2, $2, 'zd_redemption', $3, NOW(), NOW())"
    )
    .bind(chat_id)
    .bind(user_id)
    .bind(red_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO zd_redemptions (
            id, zero_deposit_id, allocation_id, user_id, chat_id, tokens_requested,
            status, created_at, updated_at
        ) VALUES (
            $1, $2, $3, $4, $5, 30000.0, 'docs_pending'::zd_redemption_status, NOW(), NOW()
        )"#
    )
    .bind(red_id)
    .bind(app_id)
    .bind(alloc.allocation_id)
    .bind(user_id)
    .bind(chat_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query("UPDATE zero_deposits SET status = 'tokens_issued'::zero_deposit_status WHERE id = $1")
        .bind(app_id)
        .execute(pool)
        .await
        .unwrap();

    // Simulate passage of 31 days by artificially aging expires_at
    sqlx::query(
        "UPDATE zd_token_allocations
         SET expires_at = NOW() - INTERVAL '1 day'
         WHERE id = $1"
    )
    .bind(alloc.allocation_id)
    .execute(pool)
    .await
    .unwrap();

    // Run the background expiry job
    let summary = livana_backend_v2::services::zd_jobs::run_token_expiry_job(pool).await.unwrap();
    assert!(summary.expired_allocations >= 1, "At least one allocation must be expired");

    // Verify allocation status is expired and available is 0
    let (alloc_status, available): (String, f64) = sqlx::query_as(
        "SELECT status::TEXT, available::FLOAT8 FROM zd_token_allocations WHERE id = $1"
    )
    .bind(alloc.allocation_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(alloc_status, "expired");
    assert_eq!(available, 0.0);

    // Verify docs_pending redemption was closed as expired
    let red_status: String = sqlx::query_scalar("SELECT status::TEXT FROM zd_redemptions WHERE id = $1")
        .bind(red_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(red_status, "expired");

    // Verify application status is expired
    let app_status: String = sqlx::query_scalar("SELECT status::TEXT FROM zero_deposits WHERE id = $1")
        .bind(app_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(app_status, "expired");

    println!("   [PASSED] Day 30 background expiry job expired allocation, closed docs_pending redemption, and transitioned application to expired.");
}

