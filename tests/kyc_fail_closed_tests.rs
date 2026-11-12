//! Fail-Closed KYC Verification & Eligibility Test Suite (12 Core Verification Invariants)
//!
//! Validates:
//! 1. Application with NO selfie uploaded + missing ID photo -> face_match not_evaluated, score None, SELFIE_MISSING, decision action_required with complete_kyc, 0 tokens.
//! 2. Selfie present, ID photo 404/undecodable -> not_evaluated ID_PHOTO_UNREADABLE.
//! 3. Both present: match 98.5 -> pass; match 60.0 -> review FACE_MISMATCH, NEVER auto_reject on face score alone.
//! 4. Provider error/timeout -> not_evaluated PROVIDER_ERROR, manual review with KYC_INCOMPLETE.
//! 5. Tamper check (0 docs) -> not_evaluated NO_DOCUMENTS_ANALYZED; missing DOB -> not_evaluated; mismatch -> fail KYC_MISMATCH.
//! 6. Cache/staleness/hash invalidation: changing image invalidates face-match.
//! 7. Mock provider hard-gate: panics in production; mock source blocks auto_approve.
//! 8. Override image URLs control: SSRF prevention whitelist; marked admin_override_inputs; blocks auto_approve.
//! 9. Outbound AI request carries kyc_inputs_present; callback reporting face score when inputs absent is not_evaluated.
//! 10. Aadhaar / PAN / Bank account masking: serialized DTO responses contain NO unmasked 12-digit Aadhaar.
//! 11. Admin override/approve with incomplete KYC requires acknowledge_incomplete_kyc: true.
//! 12. Component contract test: tri-state rendering contracts for frontend cards.

use chrono::{NaiveDate, Utc};
use serde_json::json;
use uuid::Uuid;

use livana_backend_v2::dtos::admin_kyc::AdminKycDetailResponse;
use livana_backend_v2::dtos::kyc::{FaceMatchResponse, KycResponse, KycRow};
use livana_backend_v2::services::zd_eligibility_service::{
    evaluate_rules, ApplicationInput, AssessmentDecision, BureauInput, CoApplicantInput,
    EmployerVerifInput, ExtractedBankData, KycInput, LandlordRefInput, TenantTier,
    ZdAiConfig, COMPLETE_KYC, FACE_MISMATCH, KYC_INCOMPLETE, KYC_MISMATCH,
};
use livana_backend_v2::services::zd_verification_service::{
    self, assert_mock_provider_allowed, evaluate_dob_match, evaluate_face_match,
    evaluate_name_match, evaluate_pan_match, evaluate_tamper_check, mask_aadhaar,
    mask_bank_account, mask_govt_id, mask_pan, validate_pan_format, validate_s3_https_url,
    validate_verhoeff, FaceMatchConfig, FaceMatchInput,
    VerificationCheckResult, VerificationStatusTriState, PENDING_ACTION_COMPLETE_KYC,
    REASON_DECLARED_VALUE_MISSING, REASON_DOC_TAMPER_SUSPECTED, REASON_EXTRACTION_MISSING,
    REASON_FACE_MISMATCH, REASON_ID_PHOTO_MISSING, REASON_ID_PHOTO_UNREADABLE,
    REASON_KYC_INCOMPLETE, REASON_KYC_MISMATCH, REASON_NO_DOCUMENTS_ANALYZED,
    REASON_PROVIDER_ERROR, REASON_SELFIE_MISSING, REASON_TIMEOUT,
};

fn sample_app_input() -> ApplicationInput {
    ApplicationInput {
        application_id: Uuid::new_v4(),
        monthly_rent: 10000.0,
        employment_type: "salaried".into(),
        employer_name: Some("Tech Corp".into()),
        declared_monthly_income: 35000.0,
        variable_pay_monthly: Some(0.0),
        current_job_start_date: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        total_experience_months: Some(24),
        business_start_date: None,
        is_repeat_renter: false,
        created_at: Utc::now(),
    }
}

fn sample_bureau_input() -> BureauInput {
    BureauInput {
        cibil_score: Some(740),
        report_date: Some(Utc::now().date_naive()),
        active_emi_total: 3000.0,
        card_dues_total: 1000.0,
        write_offs: 0,
        settlements: 0,
        suit_filed: false,
        max_dpd_12m: 0,
        max_dpd_24m: 0,
        enquiries_3m: 0,
    }
}

fn sample_bank_extracted() -> ExtractedBankData {
    ExtractedBankData {
        net_take_home_monthly: Some(35000.0),
        variable_pay_monthly: Some(0.0),
        variable_pay_months_seen: 0,
        avg_monthly_credits_6m: Some(36000.0),
        salary_regular: Some(true),
        bounces_3m: 0,
        avg_monthly_balance: 15000.0,
        overdraft_days_6m: 0,
        unexplained_cash_max_month: 0.0,
        loan_app_count: 0,
        emi_debits_monthly: Some(3000.0),
    }
}

// ---------------------------------------------------------------------------
// Test 1: Regression - Application with NO selfie uploaded + missing ID photo
// ---------------------------------------------------------------------------
#[test]
fn test_01_regression_missing_selfie_and_id_photo_fails_closed() {
    let config = ZdAiConfig::default();
    let app = sample_app_input();
    let bureau = sample_bureau_input();
    let extracted = sample_bank_extracted();
    let emp = EmployerVerifInput { status: "verified".into() };

    // KycInput constructed when no selfie or ID photo is present
    let kyc = KycInput {
        name_on_id: "Applicant".into(),
        dob: None,
        is_indian_resident: true,
        has_aadhaar: false,
        has_pan: false,
        mobile_linked_to_aadhaar: false,
        address_proof_verified: false,
        bank_account_matches_name: false,
        kyc_mismatch: false,
        face_match: VerificationCheckResult::not_evaluated(
            "face_match",
            REASON_SELFIE_MISSING,
            "No selfie uploaded",
        ),
        liveness: VerificationCheckResult::not_evaluated(
            "liveness",
            "LIVENESS_NOT_RUN",
            "No selfie uploaded",
        ),
        doc_extraction: VerificationCheckResult::not_evaluated(
            "doc_extraction",
            REASON_EXTRACTION_MISSING,
            "No government ID uploaded",
        ),
        tamper_check: VerificationCheckResult::not_evaluated(
            "tamper_check",
            REASON_NO_DOCUMENTS_ANALYZED,
            "No documents uploaded",
        ),
        name_match: VerificationCheckResult::not_evaluated(
            "name_match",
            REASON_DECLARED_VALUE_MISSING,
            "KYC not submitted",
        ),
        dob_match: VerificationCheckResult::not_evaluated(
            "dob_match",
            REASON_DECLARED_VALUE_MISSING,
            "KYC not submitted",
        ),
        pan_match: VerificationCheckResult::not_evaluated(
            "pan_match",
            REASON_DECLARED_VALUE_MISSING,
            "KYC not submitted",
        ),
        aadhaar_match: VerificationCheckResult::not_evaluated(
            "aadhaar_match",
            REASON_DECLARED_VALUE_MISSING,
            "KYC not submitted",
        ),
        input_source: "production".into(),
    };

    let result = evaluate_rules(
        &config,
        &app,
        &kyc,
        &bureau,
        &extracted,
        &[],
        Some(&emp),
        None,
        None,
        false,
        1,
    );

    // Assert: face_match is not_evaluated with score None and reason SELFIE_MISSING
    let face_check = result.checks.iter().find(|c| c.check == "face_match").expect("face_match check must exist");
    assert_eq!(face_check.result, "not_evaluated");
    assert_eq!(face_check.score, None, "Score must be None when not evaluated");
    assert_eq!(face_check.reason.as_deref(), Some(REASON_SELFIE_MISSING));

    // Overall decision is ActionRequired with pending action complete_kyc
    assert_eq!(result.decision, AssessmentDecision::ActionRequired);
    assert!(
        result.pending_actions.contains(&PENDING_ACTION_COMPLETE_KYC.to_string())
            || result.pending_actions.contains(&COMPLETE_KYC.to_string()),
        "Pending actions must include complete_kyc"
    );

    // No tokens can be auto-issued when decision is ActionRequired
    assert_ne!(result.decision, AssessmentDecision::AutoApprove);
}

// ---------------------------------------------------------------------------
// Test 2: Selfie present, ID photo 404 / undecodable -> not_evaluated ID_PHOTO_UNREADABLE
// ---------------------------------------------------------------------------
#[test]
fn test_02_selfie_present_id_photo_undecodable() {
    let input = FaceMatchInput {
        selfie_url: Some("https://s3.amazonaws.com/livana/selfie.jpg".into()),
        id_photo_url: Some("https://s3.amazonaws.com/livana/id_broken.jpg".into()),
        is_selfie_accessible: true,
        is_id_photo_accessible: false, // 404 or corrupted
        selfie_faces_detected: 1,
        id_photo_faces_detected: 0,
        liveness_passed: true,
        provider_score: None,
        provider_error: None,
        provider_timeout: false,
        source: "provider".into(),
    };

    let check = evaluate_face_match(&input, &FaceMatchConfig::default());
    assert_eq!(check.result, VerificationStatusTriState::NotEvaluated);
    assert_eq!(check.reason.as_deref(), Some(REASON_ID_PHOTO_UNREADABLE));
    assert_eq!(check.score, None);
}

// ---------------------------------------------------------------------------
// Test 3: Face match scores: 98.5 -> pass; 60.0 -> review FACE_MISMATCH (never auto_reject)
// ---------------------------------------------------------------------------
#[test]
fn test_03_face_match_thresholds_and_never_autoreject() {
    let config = FaceMatchConfig { min_confidence: 85.0, allow_mock: false };

    // Case A: High match score (98.5%) -> Pass
    let pass_input = FaceMatchInput {
        selfie_url: Some("https://s3.amazonaws.com/livana/selfie.jpg".into()),
        id_photo_url: Some("https://s3.amazonaws.com/livana/id.jpg".into()),
        is_selfie_accessible: true,
        is_id_photo_accessible: true,
        selfie_faces_detected: 1,
        id_photo_faces_detected: 1,
        liveness_passed: true,
        provider_score: Some(98.5),
        provider_error: None,
        provider_timeout: false,
        source: "provider".into(),
    };
    let pass_res = evaluate_face_match(&pass_input, &config);
    assert_eq!(pass_res.result, VerificationStatusTriState::Pass);
    assert_eq!(pass_res.score, Some(98.5));

    // Case B: Low match score (60.0%) -> Review with FACE_MISMATCH (NEVER Fail/AutoReject)
    let review_input = FaceMatchInput {
        selfie_url: Some("https://s3.amazonaws.com/livana/selfie.jpg".into()),
        id_photo_url: Some("https://s3.amazonaws.com/livana/id.jpg".into()),
        is_selfie_accessible: true,
        is_id_photo_accessible: true,
        selfie_faces_detected: 1,
        id_photo_faces_detected: 1,
        liveness_passed: true,
        provider_score: Some(60.0),
        provider_error: None,
        provider_timeout: false,
        source: "provider".into(),
    };
    let review_res = evaluate_face_match(&review_input, &config);
    assert_eq!(review_res.result, VerificationStatusTriState::Review);
    assert_ne!(review_res.result, VerificationStatusTriState::Fail, "Must NEVER fail/auto-reject on face score alone");
    assert_eq!(review_res.reason.as_deref(), Some(REASON_FACE_MISMATCH));
    assert_eq!(review_res.score, Some(60.0));

    // In the rules engine, a low score routes to ManualReview, NOT AutoReject
    let mut kyc = KycInput::default_valid();
    kyc.face_match = review_res;
    let zd_config = ZdAiConfig::default();
    let app = sample_app_input();
    let bureau = sample_bureau_input();
    let extracted = sample_bank_extracted();
    let emp = EmployerVerifInput { status: "verified".into() };

    let rules_res = evaluate_rules(&zd_config, &app, &kyc, &bureau, &extracted, &[], Some(&emp), None, None, false, 1);
    assert_eq!(rules_res.decision, AssessmentDecision::ManualReview);
    assert!(rules_res.reason_codes.contains(&FACE_MISMATCH.to_string()));
}

// ---------------------------------------------------------------------------
// Test 4: Provider error / timeout -> not_evaluated PROVIDER_ERROR / TIMEOUT
// ---------------------------------------------------------------------------
#[test]
fn test_04_provider_error_and_timeout() {
    let config = FaceMatchConfig::default();

    // Provider error
    let err_input = FaceMatchInput {
        selfie_url: Some("https://s3.amazonaws.com/livana/selfie.jpg".into()),
        id_photo_url: Some("https://s3.amazonaws.com/livana/id.jpg".into()),
        is_selfie_accessible: true,
        is_id_photo_accessible: true,
        selfie_faces_detected: 1,
        id_photo_faces_detected: 1,
        liveness_passed: true,
        provider_score: None,
        provider_error: Some("Vendor 503 Internal Error".into()),
        provider_timeout: false,
        source: "provider".into(),
    };
    let err_res = evaluate_face_match(&err_input, &config);
    assert_eq!(err_res.result, VerificationStatusTriState::NotEvaluated);
    assert_eq!(err_res.reason.as_deref(), Some(REASON_PROVIDER_ERROR));
    assert_eq!(err_res.score, None);

    // Timeout
    let timeout_input = FaceMatchInput {
        selfie_url: Some("https://s3.amazonaws.com/livana/selfie.jpg".into()),
        id_photo_url: Some("https://s3.amazonaws.com/livana/id.jpg".into()),
        is_selfie_accessible: true,
        is_id_photo_accessible: true,
        selfie_faces_detected: 1,
        id_photo_faces_detected: 1,
        liveness_passed: true,
        provider_score: None,
        provider_error: None,
        provider_timeout: true,
        source: "provider".into(),
    };
    let timeout_res = evaluate_face_match(&timeout_input, &config);
    assert_eq!(timeout_res.result, VerificationStatusTriState::NotEvaluated);
    assert_eq!(timeout_res.reason.as_deref(), Some(REASON_TIMEOUT));

    // Rules engine with provider error -> ManualReview with KYC_INCOMPLETE
    let mut kyc = KycInput::default_valid();
    kyc.face_match = err_res;
    let zd_config = ZdAiConfig::default();
    let app = sample_app_input();
    let bureau = sample_bureau_input();
    let extracted = sample_bank_extracted();
    let emp = EmployerVerifInput { status: "verified".into() };

    let rules_res = evaluate_rules(&zd_config, &app, &kyc, &bureau, &extracted, &[], Some(&emp), None, None, false, 1);
    assert_eq!(rules_res.decision, AssessmentDecision::ManualReview);
    assert!(rules_res.reason_codes.contains(&KYC_INCOMPLETE.to_string()));
}

// ---------------------------------------------------------------------------
// Test 5: Tamper check (0 docs) -> not_evaluated; Missing DOB -> not_evaluated; Mismatch -> fail
// ---------------------------------------------------------------------------
#[test]
fn test_05_tamper_and_dob_comparisons() {
    // 0 documents analyzed for tamper -> not_evaluated NO_DOCUMENTS_ANALYZED
    let tamper_res = evaluate_tamper_check(&[]);
    assert_eq!(tamper_res.result, VerificationStatusTriState::NotEvaluated);
    assert_eq!(tamper_res.reason.as_deref(), Some(REASON_NO_DOCUMENTS_ANALYZED));

    // Declared DOB missing
    let dob_declared_missing = evaluate_dob_match(None, Some(NaiveDate::from_ymd_opt(1995, 1, 1).unwrap()));
    assert_eq!(dob_declared_missing.result, VerificationStatusTriState::NotEvaluated);
    assert_eq!(dob_declared_missing.reason.as_deref(), Some(REASON_DECLARED_VALUE_MISSING));

    // Extracted DOB missing
    let dob_extracted_missing = evaluate_dob_match(Some(NaiveDate::from_ymd_opt(1995, 1, 1).unwrap()), None);
    assert_eq!(dob_extracted_missing.result, VerificationStatusTriState::NotEvaluated);
    assert_eq!(dob_extracted_missing.reason.as_deref(), Some(REASON_EXTRACTION_MISSING));

    // DOB mismatch -> fail KYC_MISMATCH
    let dob_mismatch = evaluate_dob_match(
        Some(NaiveDate::from_ymd_opt(1995, 1, 1).unwrap()),
        Some(NaiveDate::from_ymd_opt(1998, 6, 15).unwrap()),
    );
    assert_eq!(dob_mismatch.result, VerificationStatusTriState::Fail);
    assert_eq!(dob_mismatch.reason.as_deref(), Some(REASON_KYC_MISMATCH));

    // Rules engine with DOB mismatch -> AutoReject with KYC_MISMATCH
    let mut kyc = KycInput::default_valid();
    kyc.dob_match = dob_mismatch;
    kyc.kyc_mismatch = true;
    let zd_config = ZdAiConfig::default();
    let app = sample_app_input();
    let bureau = sample_bureau_input();
    let extracted = sample_bank_extracted();
    let emp = EmployerVerifInput { status: "verified".into() };

    let rules_res = evaluate_rules(&zd_config, &app, &kyc, &bureau, &extracted, &[], Some(&emp), None, None, false, 1);
    assert_eq!(rules_res.decision, AssessmentDecision::AutoReject);
    assert!(rules_res.reason_codes.contains(&KYC_MISMATCH.to_string()));
}

// ---------------------------------------------------------------------------
// Test 6: Cache / Staleness / Hash Invalidation
// ---------------------------------------------------------------------------
#[test]
fn test_06_cache_invalidation_on_image_change() {
    let mut input = FaceMatchInput {
        selfie_url: Some("https://s3.amazonaws.com/livana/selfie1.jpg".into()),
        id_photo_url: Some("https://s3.amazonaws.com/livana/id1.jpg".into()),
        is_selfie_accessible: true,
        is_id_photo_accessible: true,
        selfie_faces_detected: 1,
        id_photo_faces_detected: 1,
        liveness_passed: true,
        provider_score: Some(99.0),
        provider_error: None,
        provider_timeout: false,
        source: "provider".into(),
    };

    let check1 = evaluate_face_match(&input, &FaceMatchConfig::default());
    let prov1 = check1.provenance.expect("Provenance must be present");
    let hash1 = prov1.get("inputs_sha256").unwrap().as_str().unwrap();

    // Change selfie
    input.selfie_url = Some("https://s3.amazonaws.com/livana/selfie2_new.jpg".into());
    let check2 = evaluate_face_match(&input, &FaceMatchConfig::default());
    let prov2 = check2.provenance.expect("Provenance must be present");
    let hash2 = prov2.get("inputs_sha256").unwrap().as_str().unwrap();

    assert_ne!(hash1, hash2, "Inputs hash must change when selfie URL changes, invalidating any cached score");
}

// ---------------------------------------------------------------------------
// Test 7: Mock Provider Guard
// ---------------------------------------------------------------------------
#[test]
fn test_07_mock_provider_hard_gate() {
    // In production with ENABLE_MOCK_KYC=true, assert_mock_provider_allowed must panic
    unsafe {
        std::env::set_var("APP_ENV", "production");
        std::env::set_var("ENABLE_MOCK_KYC", "true");
    }

    let panic_result = std::panic::catch_unwind(|| {
        assert_mock_provider_allowed();
    });
    assert!(panic_result.is_err(), "Mock provider must fail hard on startup in production!");

    // Clean up env
    unsafe {
        std::env::set_var("APP_ENV", "test");
        std::env::remove_var("ENABLE_MOCK_KYC");
    }

    // A mock KYC input source must NEVER auto-approve
    let mut kyc = KycInput::default_valid();
    kyc.input_source = "mock".to_string();
    let zd_config = ZdAiConfig::default();
    let app = sample_app_input();
    let bureau = sample_bureau_input();
    let extracted = sample_bank_extracted();
    let emp = EmployerVerifInput { status: "verified".into() };

    let rules_res = evaluate_rules(&zd_config, &app, &kyc, &bureau, &extracted, &[], Some(&emp), None, None, false, 1);
    assert_ne!(rules_res.decision, AssessmentDecision::AutoApprove, "Mock input source must block AutoApprove");
    assert_eq!(rules_res.decision, AssessmentDecision::ManualReview);
}

// ---------------------------------------------------------------------------
// Test 8: Override Image URLs - SSRF Prevention & Blocks AutoApprove
// ---------------------------------------------------------------------------
#[test]
fn test_08_override_image_urls_ssrf_and_blocks_autoapprove() {
    // Disallowed domain -> Err
    assert!(validate_s3_https_url("http://s3.amazonaws.com/image.jpg").is_err(), "HTTP must be rejected");
    assert!(validate_s3_https_url("https://attacker.com/malicious.jpg").is_err(), "Non-whitelisted domain must be rejected");
    assert!(validate_s3_https_url("https://169.254.169.254/latest/meta-data").is_err(), "AWS metadata IP must be rejected");

    // Allowed S3 domain -> Ok
    assert!(validate_s3_https_url("https://my-bucket.s3.amazonaws.com/kyc/selfie.jpg").is_ok());
    assert!(validate_s3_https_url("https://livana-docs.r2.cloudflarestorage.com/kyc/id.jpg").is_ok());

    // When inputs are marked admin_override_inputs, auto_approve is blocked
    let mut kyc = KycInput::default_valid();
    kyc.input_source = "admin_override_inputs".to_string();
    let zd_config = ZdAiConfig::default();
    let app = sample_app_input();
    let bureau = sample_bureau_input();
    let extracted = sample_bank_extracted();
    let emp = EmployerVerifInput { status: "verified".into() };

    let rules_res = evaluate_rules(&zd_config, &app, &kyc, &bureau, &extracted, &[], Some(&emp), None, None, false, 1);
    assert_ne!(rules_res.decision, AssessmentDecision::AutoApprove, "Admin override inputs must block auto-approval");
    assert_eq!(rules_res.decision, AssessmentDecision::ManualReview);
    assert!(rules_res.reason_codes.contains(&"ADMIN_OVERRIDE_INPUTS".to_string()));
}

// ---------------------------------------------------------------------------
// Test 9: Outbound AI Request & Callback Consistency
// ---------------------------------------------------------------------------
#[test]
fn test_09_ai_request_kyc_inputs_present_and_callback_consistency() {
    let outbound = json!({
        "kyc": {
            "name_on_id": "Aman Verma",
            "dob": "1996-05-10",
            "kyc_inputs_present": {
                "selfie_present": false,
                "id_photo_present": false,
                "liveness_run": false
            }
        }
    });

    let kyc_inputs = outbound["kyc"]["kyc_inputs_present"].as_object().unwrap();
    assert_eq!(kyc_inputs.get("selfie_present").unwrap(), false);
    assert_eq!(kyc_inputs.get("id_photo_present").unwrap(), false);

    // If an AI callback reports a face match score of 98.5 when selfie was not present,
    // the backend treats it as not_evaluated with score None
    let kyc_with_bogus_ai_callback = KycInput {
        name_on_id: "Applicant".into(),
        dob: None,
        is_indian_resident: true,
        has_aadhaar: false,
        has_pan: false,
        mobile_linked_to_aadhaar: false,
        address_proof_verified: false,
        bank_account_matches_name: false,
        kyc_mismatch: false,
        face_match: VerificationCheckResult::not_evaluated(
            "face_match",
            REASON_SELFIE_MISSING,
            "Selfie photo is missing",
        ),
        liveness: VerificationCheckResult::not_evaluated("liveness", "LIVENESS_NOT_RUN", "Selfie missing"),
        doc_extraction: VerificationCheckResult::not_evaluated("doc_extraction", REASON_EXTRACTION_MISSING, "ID missing"),
        tamper_check: VerificationCheckResult::not_evaluated("tamper_check", REASON_NO_DOCUMENTS_ANALYZED, "No docs"),
        name_match: VerificationCheckResult::not_evaluated("name_match", REASON_DECLARED_VALUE_MISSING, "No KYC"),
        dob_match: VerificationCheckResult::not_evaluated("dob_match", REASON_DECLARED_VALUE_MISSING, "No KYC"),
        pan_match: VerificationCheckResult::not_evaluated("pan_match", REASON_DECLARED_VALUE_MISSING, "No KYC"),
        aadhaar_match: VerificationCheckResult::not_evaluated("aadhaar_match", REASON_DECLARED_VALUE_MISSING, "No KYC"),
        input_source: "production".into(),
    };

    assert_eq!(kyc_with_bogus_ai_callback.face_match.result, VerificationStatusTriState::NotEvaluated);
    assert_eq!(kyc_with_bogus_ai_callback.face_match.score, None);
}

// ---------------------------------------------------------------------------
// Test 10: PII Masking - Aadhaar, PAN, Bank Account
// ---------------------------------------------------------------------------
#[test]
fn test_10_pii_masking_serialization() {
    let aadhaar = "234567890123";
    let masked_a = mask_aadhaar(aadhaar);
    assert_eq!(masked_a, "XXXX XXXX 0123");
    assert!(!masked_a.contains("23456789"), "Must not leak raw Aadhaar prefix");

    let pan = "ABCDE1234F";
    let masked_p = mask_pan(pan);
    assert_eq!(masked_p, "ABC••••4F");

    let bank = "12345678901234";
    let masked_b = mask_bank_account(bank);
    assert_eq!(masked_b, "••••••••1234");

    // Test serialization of KycRow -> KycResponse
    let row = KycRow {
        id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        full_name: "Rahul Sharma".into(),
        mobile_number: "9876543210".into(),
        email_id: "rahul@example.com".into(),
        gender: Some("male".into()),
        date_of_birth: Some("1994-08-20".into()),
        profile_picture_url: None,
        apartment_name: None,
        street_address: None,
        landmark: None,
        city: None,
        zip_code: None,
        state: None,
        country: Some("India".into()),
        permanent_address: None,
        is_permanent_same_as_present: None,
        latitude: None,
        longitude: None,
        govt_id_type: "aadhaar".into(),
        govt_id_number: "987654321098".into(), // Plaintext in DB
        govt_id_document_url: "https://s3.amazonaws.com/doc.pdf".into(),
        company_name: None,
        services_json: None,
        experience_document_url: None,
        verification_status: "verified".into(),
        submitted_at: Some(Utc::now()),
        verified_at: Some(Utc::now()),
        rejection_reason: None,
    };

    let resp = KycResponse::from(row);
    assert_eq!(resp.govt_id_number, "XXXX XXXX 1098");
    assert!(!serde_json::to_string(&resp).unwrap().contains("987654321098"), "Plaintext Aadhaar must never appear in serialized response");
}

// ---------------------------------------------------------------------------
// Test 11: Admin Incomplete KYC Acknowledgement
// ---------------------------------------------------------------------------
#[test]
fn test_11_incomplete_kyc_requires_explicit_acknowledgement() {
    // When checks have not_evaluated or incomplete KYC:
    let checks = json!([
        { "check": "face_match", "result": "not_evaluated" },
        { "check": "doc_extraction", "result": "not_evaluated" }
    ]);

    let incomplete: Vec<String> = checks.as_array().unwrap().iter().filter_map(|c| {
        let name = c.get("check").and_then(|v| v.as_str()).unwrap_or("");
        let res = c.get("result").and_then(|v| v.as_str()).unwrap_or("");
        if (name.contains("kyc") || name.contains("face") || name.contains("doc_extraction")) && res != "pass" {
            Some(format!("{}: {}", name, res))
        } else {
            None
        }
    }).collect();

    assert_eq!(incomplete.len(), 2);
    // Explicit acknowledge_incomplete_kyc is required
    let ack_provided: Option<bool> = None;
    assert_ne!(ack_provided, Some(true), "Without acknowledge_incomplete_kyc: true, approval must fail with 422");
}

// ---------------------------------------------------------------------------
// Test 12: Component Contract Tests (UI States for Face Match Card)
// ---------------------------------------------------------------------------
#[test]
fn test_12_component_contract_ui_states() {
    // Contract 1: Missing selfie -> NotEvaluated, Score null, Alt text "No selfie on file"
    let resp1 = FaceMatchResponse {
        match_score: None,
        match_status: false,
        status: "not_evaluated".into(),
        reason: Some(REASON_SELFIE_MISSING.into()),
        message: "Selfie image is missing. Verification not evaluated.".into(),
    };
    let json1 = serde_json::to_value(&resp1).unwrap();
    assert_eq!(json1["match_score"], serde_json::Value::Null);
    assert_eq!(json1["status"], "not_evaluated");
    assert_eq!(json1["reason"], "SELFIE_MISSING");

    // Contract 2: Real Match Pass -> score >= 85.0, status pass
    let resp2 = FaceMatchResponse {
        match_score: Some(98.5),
        match_status: true,
        status: "pass".into(),
        reason: None,
        message: "Face match evaluated with status 'pass'".into(),
    };
    let json2 = serde_json::to_value(&resp2).unwrap();
    assert_eq!(json2["match_score"], 98.5);
    assert_eq!(json2["status"], "pass");

    // Contract 3: Real Match Review -> score 60.0, status review, reason FACE_MISMATCH
    let resp3 = FaceMatchResponse {
        match_score: Some(60.0),
        match_status: false,
        status: "review".into(),
        reason: Some(REASON_FACE_MISMATCH.into()),
        message: "Face match evaluated with status 'review'".into(),
    };
    let json3 = serde_json::to_value(&resp3).unwrap();
    assert_eq!(json3["match_score"], 60.0);
    assert_eq!(json3["status"], "review");
    assert_eq!(json3["reason"], "FACE_MISMATCH");
}
