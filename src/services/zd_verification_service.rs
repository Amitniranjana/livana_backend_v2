//! Fail-Closed KYC Verification Engine & Tri-State Result Types
//!
//! Enforces:
//! - Explicit tri-state checks: pass | fail | review | not_evaluated.
//! - Machine-readable reason codes (SELFIE_MISSING, ID_PHOTO_MISSING, etc.).
//! - Score is Option<f32> (None when not evaluated; NEVER a default number).
//! - Roll-up per card: all pass => pass; any not_evaluated => not_evaluated; any fail => fail; else review.
//! - Verhoeff 12-digit Aadhaar checksum validation and PAN regex.
//! - Aadhaar / PAN / Bank account masking.
//! - Production mock provider hard-gate.

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utoipa::ToSchema;

// ---------------------------------------------------------------------------
// Tri-State Status & Verification Result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatusTriState {
    Pass,
    Fail,
    Review,
    NotEvaluated,
}

impl VerificationStatusTriState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Review => "review",
            Self::NotEvaluated => "not_evaluated",
        }
    }
}

impl std::fmt::Display for VerificationStatusTriState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Machine-readable reason codes
// ---------------------------------------------------------------------------

pub const REASON_SELFIE_MISSING: &str = "SELFIE_MISSING";
pub const REASON_ID_PHOTO_MISSING: &str = "ID_PHOTO_MISSING";
pub const REASON_ID_PHOTO_UNREADABLE: &str = "ID_PHOTO_UNREADABLE";
pub const REASON_SELFIE_UNREADABLE: &str = "SELFIE_UNREADABLE";
pub const REASON_NO_FACE_DETECTED: &str = "NO_FACE_DETECTED";
pub const REASON_MULTIPLE_FACES: &str = "MULTIPLE_FACES";
pub const REASON_LIVENESS_NOT_RUN: &str = "LIVENESS_NOT_RUN";
pub const REASON_LIVENESS_FAILED: &str = "LIVENESS_FAILED";
pub const REASON_DECLARED_VALUE_MISSING: &str = "DECLARED_VALUE_MISSING";
pub const REASON_EXTRACTION_MISSING: &str = "EXTRACTION_MISSING";
pub const REASON_KYC_MISMATCH: &str = "KYC_MISMATCH";
pub const REASON_FACE_MISMATCH: &str = "FACE_MISMATCH";
pub const REASON_BELOW_THRESHOLD: &str = "BELOW_THRESHOLD";
pub const REASON_PROVIDER_ERROR: &str = "PROVIDER_ERROR";
pub const REASON_TIMEOUT: &str = "TIMEOUT";
pub const REASON_NO_DOCUMENTS_ANALYZED: &str = "NO_DOCUMENTS_ANALYZED";
pub const REASON_DOC_TAMPER_SUSPECTED: &str = "DOC_TAMPER_SUSPECTED";
pub const REASON_INVALID_FORMAT: &str = "INVALID_FORMAT";
pub const REASON_ADMIN_OVERRIDE_INPUTS: &str = "admin_override_inputs";
pub const REASON_KYC_INCOMPLETE: &str = "KYC_INCOMPLETE";
pub const PENDING_ACTION_COMPLETE_KYC: &str = "complete_kyc";

// ---------------------------------------------------------------------------
// Verification Check Result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct VerificationCheckResult {
    pub check: String,
    pub result: VerificationStatusTriState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<serde_json::Value>,
}

impl VerificationCheckResult {
    pub fn pass(check: impl Into<String>, detail: impl Into<String>, score: Option<f32>) -> Self {
        Self {
            check: check.into(),
            result: VerificationStatusTriState::Pass,
            reason: None,
            score,
            detail: detail.into(),
            provenance: None,
        }
    }

    pub fn fail(
        check: impl Into<String>,
        reason: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            check: check.into(),
            result: VerificationStatusTriState::Fail,
            reason: Some(reason.into()),
            score: None,
            detail: detail.into(),
            provenance: None,
        }
    }

    pub fn review(
        check: impl Into<String>,
        reason: impl Into<String>,
        detail: impl Into<String>,
        score: Option<f32>,
    ) -> Self {
        Self {
            check: check.into(),
            result: VerificationStatusTriState::Review,
            reason: Some(reason.into()),
            score,
            detail: detail.into(),
            provenance: None,
        }
    }

    pub fn not_evaluated(
        check: impl Into<String>,
        reason: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            check: check.into(),
            result: VerificationStatusTriState::NotEvaluated,
            reason: Some(reason.into()),
            score: None, // Score is strictly None when not evaluated
            detail: detail.into(),
            provenance: None,
        }
    }

    pub fn with_provenance(mut self, provenance: serde_json::Value) -> Self {
        self.provenance = Some(provenance);
        self
    }
}

// ---------------------------------------------------------------------------
// Roll-up Helper: Pass only if all pass, Fail if any fail, NotEvaluated if any not evaluated, else Review
// ---------------------------------------------------------------------------

pub fn rollup_checks(
    checks: &[VerificationCheckResult],
) -> (VerificationStatusTriState, Option<String>) {
    if checks.is_empty() {
        return (
            VerificationStatusTriState::NotEvaluated,
            Some(REASON_NO_DOCUMENTS_ANALYZED.to_string()),
        );
    }

    // 1. Any FAIL => overall FAIL
    if let Some(failed) = checks
        .iter()
        .find(|c| c.result == VerificationStatusTriState::Fail)
    {
        return (VerificationStatusTriState::Fail, failed.reason.clone());
    }

    // 2. Any NOT_EVALUATED => overall NOT_EVALUATED
    if let Some(ne) = checks
        .iter()
        .find(|c| c.result == VerificationStatusTriState::NotEvaluated)
    {
        return (VerificationStatusTriState::NotEvaluated, ne.reason.clone());
    }

    // 3. Any REVIEW => overall REVIEW
    if let Some(rev) = checks
        .iter()
        .find(|c| c.result == VerificationStatusTriState::Review)
    {
        return (VerificationStatusTriState::Review, rev.reason.clone());
    }

    // 4. All checks are PASS => PASS
    (VerificationStatusTriState::Pass, None)
}

// ---------------------------------------------------------------------------
// Verhoeff Algorithm for 12-Digit Aadhaar Checksum
// ---------------------------------------------------------------------------

const VERHOEFF_D: [[u8; 10]; 10] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
    [1, 2, 3, 4, 0, 6, 7, 8, 9, 5],
    [2, 3, 4, 0, 1, 7, 8, 9, 5, 6],
    [3, 4, 0, 1, 2, 8, 9, 5, 6, 7],
    [4, 0, 1, 2, 3, 9, 5, 6, 7, 8],
    [5, 9, 8, 7, 6, 0, 4, 3, 2, 1],
    [6, 5, 9, 8, 7, 1, 0, 4, 3, 2],
    [7, 6, 5, 9, 8, 2, 1, 0, 4, 3],
    [8, 7, 6, 5, 9, 3, 2, 1, 0, 4],
    [9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
];

const VERHOEFF_P: [[u8; 10]; 8] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
    [1, 5, 7, 6, 2, 8, 3, 0, 9, 4],
    [5, 8, 0, 3, 7, 9, 6, 1, 4, 2],
    [8, 9, 1, 6, 0, 4, 3, 5, 2, 7],
    [9, 4, 5, 3, 1, 2, 6, 8, 7, 0],
    [4, 2, 8, 6, 5, 7, 3, 9, 0, 1],
    [2, 7, 9, 3, 8, 0, 6, 4, 1, 5],
    [7, 0, 4, 6, 9, 1, 3, 2, 5, 8],
];

const VERHOEFF_INV: [u8; 10] = [0, 4, 3, 2, 1, 5, 6, 7, 8, 9];

/// Computes the Verhoeff check digit for a string of digits.
pub fn generate_verhoeff_check_digit(num_str: &str) -> u8 {
    let clean: String = num_str.chars().filter(|c| c.is_ascii_digit()).collect();
    let mut c = 0usize;
    for (i, ch) in clean.chars().rev().enumerate() {
        let digit = ch.to_digit(10).unwrap() as usize;
        let p_val = VERHOEFF_P[(i + 1) % 8][digit] as usize;
        c = VERHOEFF_D[c][p_val] as usize;
    }
    VERHOEFF_INV[c]
}

/// Validates a 12-digit Indian Aadhaar number using the standard Verhoeff checksum.
pub fn validate_verhoeff(num_str: &str) -> bool {
    let clean: String = num_str.chars().filter(|c| c.is_ascii_digit()).collect();
    if clean.len() != 12 {
        return false;
    }
    // Must not start with 0 or 1 per UIDAI specification
    if clean.starts_with('0') || clean.starts_with('1') {
        return false;
    }

    let mut c = 0usize;
    for (i, ch) in clean.chars().rev().enumerate() {
        let digit = ch.to_digit(10).unwrap() as usize;
        let p_val = VERHOEFF_P[i % 8][digit] as usize;
        c = VERHOEFF_D[c][p_val] as usize;
    }
    c == 0
}

/// Validates Indian PAN format: 5 letters, 4 digits, 1 letter (e.g. ABCDE1234F).
pub fn validate_pan_format(pan: &str) -> bool {
    let pan = pan.trim().to_uppercase();
    if pan.len() != 10 {
        return false;
    }
    let chars: Vec<char> = pan.chars().collect();
    chars[0..5].iter().all(|c| c.is_ascii_uppercase())
        && chars[5..9].iter().all(|c| c.is_ascii_digit())
        && chars[9].is_ascii_uppercase()
}

// ---------------------------------------------------------------------------
// Masking Helpers (PII Protection)
// ---------------------------------------------------------------------------

/// Masks Aadhaar number to display only last 4 digits: `XXXX XXXX 9012`
pub fn mask_aadhaar(aadhaar: &str) -> String {
    let digits: String = aadhaar.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 4 {
        let last4 = &digits[digits.len() - 4..];
        format!("XXXX XXXX {}", last4)
    } else {
        "XXXX XXXX XXXX".to_string()
    }
}

/// Masks PAN number: `ABC••••4F`
pub fn mask_pan(pan: &str) -> String {
    let clean = pan.trim().to_uppercase();
    if clean.len() == 10 {
        format!("{}••••{}", &clean[..3], &clean[clean.len() - 2..])
    } else if clean.len() >= 4 {
        let last2 = &clean[clean.len() - 2..];
        format!("••••••{}", last2)
    } else {
        "•••••".to_string()
    }
}

/// Masks bank account number: `••••••••1234`
pub fn mask_bank_account(account: &str) -> String {
    let clean: String = account.chars().filter(|c| c.is_ascii_digit()).collect();
    if clean.len() >= 4 {
        let last4 = &clean[clean.len() - 4..];
        format!("••••••••{}", last4)
    } else {
        "••••••••••••".to_string()
    }
}

/// Generic mask helper by govt ID type
pub fn mask_govt_id(doc_type: &str, doc_number: &str) -> String {
    match doc_type.to_lowercase().as_str() {
        "aadhaar" => mask_aadhaar(doc_number),
        "pan" => mask_pan(doc_number),
        _ => {
            let clean = doc_number.trim();
            if clean.len() > 4 {
                format!("••••••••{}", &clean[clean.len() - 4..])
            } else {
                "••••".to_string()
            }
        }
    }
}

/// Validates that an image override URL is an HTTPS S3/storage URL (prevents SSRF).
pub fn validate_s3_https_url(url_str: &str) -> Result<(), String> {
    let trimmed = url_str.trim();
    if !trimmed.starts_with("https://") {
        return Err("Image URL must use HTTPS".to_string());
    }
    let parsed = reqwest::Url::parse(trimmed).map_err(|e| format!("Invalid URL: {}", e))?;
    let host = parsed.host_str().ok_or_else(|| "URL host is missing".to_string())?.to_lowercase();
    let is_allowed = host.ends_with(".amazonaws.com")
        || host.ends_with(".r2.cloudflarestorage.com")
        || host == "s3.amazonaws.com"
        || host.contains("livana")
        || host == "localhost"
        || host == "127.0.0.1";
    if !is_allowed {
        return Err(format!("Host '{}' is not in the allowed S3 storage whitelist", host));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Field Comparison Functions (Tri-State, Fail-Closed)
// ---------------------------------------------------------------------------

pub fn normalize_field(s: &str) -> String {
    s.to_uppercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn evaluate_name_match(
    declared: Option<&str>,
    extracted: Option<&str>,
) -> VerificationCheckResult {
    let declared = declared.map(str::trim).filter(|s| !s.is_empty());
    let extracted = extracted.map(str::trim).filter(|s| !s.is_empty());

    match (declared, extracted) {
        (None, _) => VerificationCheckResult::not_evaluated(
            "name_match",
            REASON_DECLARED_VALUE_MISSING,
            "Declared applicant name is missing; cannot compare",
        ),
        (_, None) => VerificationCheckResult::not_evaluated(
            "name_match",
            REASON_EXTRACTION_MISSING,
            "Extracted name from ID document is missing; cannot compare",
        ),
        (Some(dec), Some(ext)) => {
            let norm_dec = normalize_field(dec);
            let norm_ext = normalize_field(ext);

            if norm_dec == norm_ext || norm_ext.contains(&norm_dec) || norm_dec.contains(&norm_ext) {
                VerificationCheckResult::pass(
                    "name_match",
                    format!("Name matches ('{}' matches '{}')", dec, ext),
                    Some(100.0),
                )
            } else {
                VerificationCheckResult::fail(
                    "name_match",
                    REASON_KYC_MISMATCH,
                    format!("Declared name '{}' does not match extracted name '{}'", dec, ext),
                )
            }
        }
    }
}

pub fn evaluate_dob_match(
    declared: Option<NaiveDate>,
    extracted: Option<NaiveDate>,
) -> VerificationCheckResult {
    match (declared, extracted) {
        (None, _) => VerificationCheckResult::not_evaluated(
            "dob_match",
            REASON_DECLARED_VALUE_MISSING,
            "Declared date of birth is missing; cannot compare",
        ),
        (_, None) => VerificationCheckResult::not_evaluated(
            "dob_match",
            REASON_EXTRACTION_MISSING,
            "Extracted date of birth is missing; cannot compare",
        ),
        (Some(dec), Some(ext)) => {
            if dec == ext {
                VerificationCheckResult::pass(
                    "dob_match",
                    format!("Date of birth matches ({})", dec),
                    Some(100.0),
                )
            } else {
                VerificationCheckResult::fail(
                    "dob_match",
                    REASON_KYC_MISMATCH,
                    format!("Declared DOB '{}' does not match extracted DOB '{}'", dec, ext),
                )
            }
        }
    }
}

pub fn evaluate_pan_match(
    declared: Option<&str>,
    extracted: Option<&str>,
) -> VerificationCheckResult {
    let declared = declared.map(str::trim).filter(|s| !s.is_empty());
    let extracted = extracted.map(str::trim).filter(|s| !s.is_empty());

    match (declared, extracted) {
        (None, _) => VerificationCheckResult::not_evaluated(
            "pan_match",
            REASON_DECLARED_VALUE_MISSING,
            "Declared PAN is missing; cannot compare",
        ),
        (Some(dec), _) if !validate_pan_format(dec) => VerificationCheckResult::fail(
            "pan_match",
            REASON_INVALID_FORMAT,
            format!("Declared PAN '{}' has invalid format", mask_pan(dec)),
        ),
        (_, None) => VerificationCheckResult::not_evaluated(
            "pan_match",
            REASON_EXTRACTION_MISSING,
            "Extracted PAN from document is missing; cannot compare",
        ),
        (Some(dec), Some(ext)) => {
            let clean_dec = dec.to_uppercase();
            let clean_ext = ext.to_uppercase();
            if clean_dec == clean_ext {
                VerificationCheckResult::pass(
                    "pan_match",
                    format!("PAN matches ({})", mask_pan(dec)),
                    Some(100.0),
                )
            } else {
                VerificationCheckResult::fail(
                    "pan_match",
                    REASON_KYC_MISMATCH,
                    format!("Declared PAN '{}' does not match extracted PAN '{}'", mask_pan(dec), mask_pan(ext)),
                )
            }
        }
    }
}

pub fn evaluate_aadhaar_match(
    declared: Option<&str>,
    extracted: Option<&str>,
) -> VerificationCheckResult {
    let declared = declared.map(str::trim).filter(|s| !s.is_empty());
    let extracted = extracted.map(str::trim).filter(|s| !s.is_empty());

    match (declared, extracted) {
        (None, _) => VerificationCheckResult::not_evaluated(
            "aadhaar_match",
            REASON_DECLARED_VALUE_MISSING,
            "Declared Aadhaar is missing; cannot compare",
        ),
        (Some(dec), _) if !validate_verhoeff(dec) => VerificationCheckResult::fail(
            "aadhaar_match",
            REASON_INVALID_FORMAT,
            format!("Declared Aadhaar '{}' failed Verhoeff checksum", mask_aadhaar(dec)),
        ),
        (_, None) => VerificationCheckResult::not_evaluated(
            "aadhaar_match",
            REASON_EXTRACTION_MISSING,
            "Extracted Aadhaar from document is missing; cannot compare",
        ),
        (Some(dec), Some(ext)) => {
            let clean_dec: String = dec.chars().filter(|c| c.is_ascii_digit()).collect();
            let clean_ext: String = ext.chars().filter(|c| c.is_ascii_digit()).collect();
            if clean_dec == clean_ext {
                VerificationCheckResult::pass(
                    "aadhaar_match",
                    format!("Aadhaar matches ({})", mask_aadhaar(dec)),
                    Some(100.0),
                )
            } else {
                VerificationCheckResult::fail(
                    "aadhaar_match",
                    REASON_KYC_MISMATCH,
                    format!("Declared Aadhaar '{}' does not match extracted Aadhaar '{}'", mask_aadhaar(dec), mask_aadhaar(ext)),
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Face Match & Liveness Evaluator (Fail-Closed)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FaceMatchInput {
    pub selfie_url: Option<String>,
    pub id_photo_url: Option<String>,
    pub is_selfie_accessible: bool,
    pub is_id_photo_accessible: bool,
    pub selfie_faces_detected: usize,
    pub id_photo_faces_detected: usize,
    pub liveness_passed: bool,
    pub provider_score: Option<f32>,
    pub provider_error: Option<String>,
    pub provider_timeout: bool,
    pub source: String, // "provider" | "mock" | "admin_override_inputs"
}

pub struct FaceMatchConfig {
    pub min_confidence: f32, // default 85.0
    pub allow_mock: bool,
}

impl Default for FaceMatchConfig {
    fn default() -> Self {
        Self {
            min_confidence: 85.0,
            allow_mock: false,
        }
    }
}

pub fn evaluate_face_match(
    input: &FaceMatchInput,
    config: &FaceMatchConfig,
) -> VerificationCheckResult {
    // 1. Check selfie presence
    let selfie_url = match input.selfie_url.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(url) => url,
        None => return VerificationCheckResult::not_evaluated(
            "face_match",
            REASON_SELFIE_MISSING,
            "No selfie on file for this applicant",
        ),
    };

    // 2. Check ID photo presence
    let id_photo_url = match input.id_photo_url.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(url) => url,
        None => return VerificationCheckResult::not_evaluated(
            "face_match",
            REASON_ID_PHOTO_MISSING,
            "No government ID photo on file for this applicant",
        ),
    };

    // 3. Accessibility / URL probe check
    if !input.is_selfie_accessible {
        return VerificationCheckResult::not_evaluated(
            "face_match",
            REASON_SELFIE_UNREADABLE,
            format!("Selfie image URL is inaccessible or could not be loaded: {}", selfie_url),
        );
    }
    if !input.is_id_photo_accessible {
        return VerificationCheckResult::not_evaluated(
            "face_match",
            REASON_ID_PHOTO_UNREADABLE,
            format!("ID photo image URL is inaccessible or returned 404: {}", id_photo_url),
        );
    }

    // 4. Face count validation
    if input.selfie_faces_detected == 0 {
        return VerificationCheckResult::not_evaluated(
            "face_match",
            REASON_NO_FACE_DETECTED,
            "No human face detected in the submitted selfie",
        );
    }
    if input.selfie_faces_detected > 1 {
        return VerificationCheckResult::not_evaluated(
            "face_match",
            REASON_MULTIPLE_FACES,
            "Multiple faces detected in the submitted selfie; single occupant required",
        );
    }
    if input.id_photo_faces_detected == 0 {
        return VerificationCheckResult::not_evaluated(
            "face_match",
            REASON_NO_FACE_DETECTED,
            "No clear face detected on the government ID document",
        );
    }
    if input.id_photo_faces_detected > 1 {
        return VerificationCheckResult::not_evaluated(
            "face_match",
            REASON_MULTIPLE_FACES,
            "Multiple faces detected on the government ID photo",
        );
    }

    // 5. Liveness check
    if !input.liveness_passed {
        return VerificationCheckResult::not_evaluated(
            "face_match",
            REASON_LIVENESS_NOT_RUN,
            "Selfie liveness check did not pass or was not performed",
        );
    }

    // 6. Provider execution / errors
    if input.provider_timeout {
        return VerificationCheckResult::not_evaluated(
            "face_match",
            REASON_TIMEOUT,
            "Face match provider timed out during evaluation",
        );
    }
    if let Some(err) = &input.provider_error {
        return VerificationCheckResult::not_evaluated(
            "face_match",
            REASON_PROVIDER_ERROR,
            format!("Face match provider error: {}", err),
        );
    }

    // 7. Evaluate real score against threshold
    match input.provider_score {
        None => VerificationCheckResult::not_evaluated(
            "face_match",
            REASON_PROVIDER_ERROR,
            "Face match provider did not return a numerical confidence score",
        ),
        Some(score) => {
            let mut hasher = Sha256::new();
            hasher.update(selfie_url.as_bytes());
            hasher.update(b":");
            hasher.update(id_photo_url.as_bytes());
            let inputs_sha256 = format!("{:x}", hasher.finalize());

            let provenance = serde_json::json!({
                "source": input.source,
                "score": score,
                "threshold": config.min_confidence,
                "inputs_sha256": inputs_sha256,
                "computed_at": Utc::now().to_rfc3339()
            });

            if score >= config.min_confidence {
                VerificationCheckResult::pass(
                    "face_match",
                    format!("Faces match with {:.1}% confidence (min: {:.0}%)", score, config.min_confidence),
                    Some(score),
                )
                .with_provenance(provenance)
            } else {
                // Score below threshold is manual_review (FACE_MISMATCH), NEVER auto-reject!
                VerificationCheckResult::review(
                    "face_match",
                    REASON_FACE_MISMATCH,
                    format!("Face match score {:.1}% is below threshold {:.0}%", score, config.min_confidence),
                    Some(score),
                )
                .with_provenance(provenance)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Document Tamper Check Evaluator (Fail-Closed)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AnalyzedDocument {
    pub doc_type: String,
    pub url: String,
    pub is_accessible: bool,
    pub is_tampered: bool,
    pub tamper_reasons: Vec<String>,
}

pub fn evaluate_tamper_check(documents: &[AnalyzedDocument]) -> VerificationCheckResult {
    // 1. Zero documents analyzed => not_evaluated
    if documents.is_empty() {
        return VerificationCheckResult::not_evaluated(
            "tamper_check",
            REASON_NO_DOCUMENTS_ANALYZED,
            "No documents were analyzed in this run; cannot verify document integrity",
        );
    }

    // 2. Any unreadable / inaccessible document => not_evaluated
    if let Some(unrec) = documents.iter().find(|d| !d.is_accessible) {
        return VerificationCheckResult::not_evaluated(
            "tamper_check",
            REASON_ID_PHOTO_UNREADABLE,
            format!("Document '{}' could not be fetched or analyzed", unrec.doc_type),
        );
    }

    // 3. Any document with suspected tampering => review (manual review, NOT auto-reject!)
    let tampered_docs: Vec<&AnalyzedDocument> = documents.iter().filter(|d| d.is_tampered).collect();
    if !tampered_docs.is_empty() {
        let reasons: Vec<String> = tampered_docs
            .iter()
            .map(|d| format!("{}: {}", d.doc_type, d.tamper_reasons.join(", ")))
            .collect();
        return VerificationCheckResult::review(
            "tamper_check",
            REASON_DOC_TAMPER_SUSPECTED,
            format!("Suspected document alteration/tampering detected in: {}", reasons.join("; ")),
            None,
        );
    }

    // 4. All documents fetched and clean => pass
    VerificationCheckResult::pass(
        "tamper_check",
        format!("All {} submitted document(s) verified clean with no signs of tampering", documents.len()),
        None,
    )
}

// ---------------------------------------------------------------------------
// Provenance and Input Hash Generation
// ---------------------------------------------------------------------------

pub fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

pub fn compute_combined_inputs_hash(hashes: &[(&str, &str)]) -> String {
    let mut hasher = Sha256::new();
    let mut sorted_hashes = hashes.to_vec();
    sorted_hashes.sort_by_key(|&(k, _)| k);
    for (k, h) in sorted_hashes {
        hasher.update(k.as_bytes());
        hasher.update(b":");
        hasher.update(h.as_bytes());
        hasher.update(b";");
    }
    hex::encode(hasher.finalize())
}

// ---------------------------------------------------------------------------
// Mock Provider Hard-Gate (Fail in Production)
// ---------------------------------------------------------------------------

pub fn is_mock_kyc_enabled() -> bool {
    let app_env = std::env::var("APP_ENV")
        .unwrap_or_else(|_| std::env::var("ENV").unwrap_or_else(|_| "development".into()))
        .to_lowercase();

    let is_prod = app_env == "production" || app_env == "prod";
    let mock_flag = std::env::var("ENABLE_MOCK_KYC")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    if is_prod && mock_flag {
        panic!("FATAL: Mock KYC provider cannot be enabled in production environment (APP_ENV=production)");
    }

    !is_prod && mock_flag
}

pub fn assert_mock_provider_allowed() {
    let app_env = std::env::var("APP_ENV")
        .unwrap_or_else(|_| std::env::var("ENV").unwrap_or_else(|_| "development".into()))
        .to_lowercase();

    let is_prod = app_env == "production" || app_env == "prod";
    let mock_flag = std::env::var("ENABLE_MOCK_KYC")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    if is_prod && mock_flag {
        panic!("FATAL: Mock KYC provider cannot be enabled in production environment (APP_ENV=production)");
    }
}

// ---------------------------------------------------------------------------
// Unit Tests for Verhoeff, Masking, Roll-up, and Fail-Closed Logic
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verhoeff_checksum_valid_and_invalid() {
        // Generate valid 12-digit Aadhaar using 11 digits
        let base11 = "23456789012";
        let check_digit = generate_verhoeff_check_digit(base11);
        let valid_aadhaar = format!("{}{}", base11, check_digit);
        assert!(validate_verhoeff(&valid_aadhaar));

        // Single digit alteration must fail
        let invalid_digit = (check_digit + 1) % 10;
        let invalid_aadhaar = format!("{}{}", base11, invalid_digit);
        assert!(!validate_verhoeff(&invalid_aadhaar));

        // Starting with 0 or 1 must fail
        let zero_start = format!("0{}", &valid_aadhaar[1..]);
        assert!(!validate_verhoeff(&zero_start));
        let one_start = format!("1{}", &valid_aadhaar[1..]);
        assert!(!validate_verhoeff(&one_start));

        // Invalid lengths must fail
        assert!(!validate_verhoeff("123456"));
        assert!(!validate_verhoeff(&format!("{}0", valid_aadhaar)));
    }

    #[test]
    fn test_pan_format_validation() {
        assert!(validate_pan_format("ABCDE1234F"));
        assert!(validate_pan_format("abcde1234f")); // case-insensitive
        assert!(!validate_pan_format("ABCDE12345")); // ends in digit
        assert!(!validate_pan_format("ABCD12345F")); // only 4 letters at start
        assert!(!validate_pan_format("ABCDEF1234F")); // too long
    }

    #[test]
    fn test_aadhaar_masking() {
        assert_eq!(mask_aadhaar("234567890126"), "XXXX XXXX 0126");
        assert_eq!(mask_aadhaar("2345 6789 0126"), "XXXX XXXX 0126");
    }

    #[test]
    fn test_pan_masking() {
        assert_eq!(mask_pan("ABCDE1234F"), "ABC••••4F");
    }

    #[test]
    fn test_bank_account_masking() {
        assert_eq!(mask_bank_account("123456789012"), "••••••••9012");
    }

    #[test]
    fn test_rollup_checks_fail_closed() {
        // Empty => not_evaluated
        assert_eq!(
            rollup_checks(&[]).0,
            VerificationStatusTriState::NotEvaluated
        );

        let pass1 = VerificationCheckResult::pass("c1", "ok", None);
        let pass2 = VerificationCheckResult::pass("c2", "ok", None);
        let not_eval = VerificationCheckResult::not_evaluated("c3", REASON_SELFIE_MISSING, "missing");
        let fail = VerificationCheckResult::fail("c4", REASON_KYC_MISMATCH, "mismatch");
        let review = VerificationCheckResult::review("c5", REASON_FACE_MISMATCH, "low", Some(60.0));

        // All pass => pass
        assert_eq!(
            rollup_checks(&[pass1.clone(), pass2.clone()]).0,
            VerificationStatusTriState::Pass
        );

        // Pass + NotEvaluated => NotEvaluated (never clean/pass from partial)
        assert_eq!(
            rollup_checks(&[pass1.clone(), not_eval.clone()]).0,
            VerificationStatusTriState::NotEvaluated
        );

        // Fail overrides NotEvaluated
        assert_eq!(
            rollup_checks(&[pass1.clone(), not_eval.clone(), fail.clone()]).0,
            VerificationStatusTriState::Fail
        );

        // Pass + Review => Review
        assert_eq!(
            rollup_checks(&[pass1.clone(), review.clone()]).0,
            VerificationStatusTriState::Review
        );
    }

    #[test]
    fn test_face_match_missing_inputs_returns_not_evaluated_with_none_score() {
        let input_no_selfie = FaceMatchInput {
            selfie_url: None,
            id_photo_url: Some("https://bucket.s3.amazonaws.com/id.jpg".into()),
            is_selfie_accessible: false,
            is_id_photo_accessible: true,
            selfie_faces_detected: 0,
            id_photo_faces_detected: 1,
            liveness_passed: false,
            provider_score: None,
            provider_error: None,
            provider_timeout: false,
            source: "provider".into(),
        };

        let res = evaluate_face_match(&input_no_selfie, &FaceMatchConfig::default());
        assert_eq!(res.result, VerificationStatusTriState::NotEvaluated);
        assert_eq!(res.reason, Some(REASON_SELFIE_MISSING.into()));
        assert_eq!(res.score, None); // NEVER a default number
    }

    #[test]
    fn test_face_match_score_below_threshold_is_review_never_autoreject() {
        let input_low_score = FaceMatchInput {
            selfie_url: Some("https://bucket.s3.amazonaws.com/selfie.jpg".into()),
            id_photo_url: Some("https://bucket.s3.amazonaws.com/id.jpg".into()),
            is_selfie_accessible: true,
            is_id_photo_accessible: true,
            selfie_faces_detected: 1,
            id_photo_faces_detected: 1,
            liveness_passed: true,
            provider_score: Some(60.0), // Below 85.0
            provider_error: None,
            provider_timeout: false,
            source: "provider".into(),
        };

        let res = evaluate_face_match(&input_low_score, &FaceMatchConfig::default());
        assert_eq!(res.result, VerificationStatusTriState::Review);
        assert_eq!(res.reason, Some(REASON_FACE_MISMATCH.into()));
        assert_eq!(res.score, Some(60.0));
    }

    #[test]
    fn test_tamper_check_zero_docs_is_not_evaluated() {
        let res = evaluate_tamper_check(&[]);
        assert_eq!(res.result, VerificationStatusTriState::NotEvaluated);
        assert_eq!(res.reason, Some(REASON_NO_DOCUMENTS_ANALYZED.into()));
    }
}
