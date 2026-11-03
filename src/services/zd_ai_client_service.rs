//! Issue 111 — Credit Bureau Pull at Consent
//! Issue 112 — AI Service Client + Redis Queue
//! Issue 114 — AI Document-Check Client
//!
//! Bureau pull happens when consent is given (credit_check_consent handler calls
//! pull_and_store_bureau_report). The AI job is then queued.
//!
//! For Issue 111: Bureau provider decision is pending — using a stub that
//! generates a realistic summary structure. Wire up the real provider by replacing
//! `fetch_bureau_report_from_provider`.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::time::Duration;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Bureau types (Issue 111)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BureauSummary {
    pub score: Option<i32>,
    /// Days past due in last 12 months
    pub dpd_12m: Option<i32>,
    /// Days past due in last 24 months
    pub dpd_24m: Option<i32>,
    /// Credit enquiries in last 3 months
    pub enquiries_3m: Option<i32>,
    /// Total active EMI obligations per month
    pub emi_total_monthly: Option<f64>,
    /// Total outstanding card dues
    pub card_dues: Option<f64>,
    /// Count of written-off accounts
    pub write_offs: Option<i32>,
    /// Count of settlement arrangements
    pub settlements: Option<i32>,
    /// Whether any suit has been filed against the applicant
    pub suit_filed: bool,
    /// Name of bureau provider used
    pub provider: String,
    /// Raw provider reference for audit
    pub reference_id: Option<String>,
}

/// Pull bureau report for a main applicant and store it on the application.
/// Also triggers the AI assessment queue (Issue 112).
pub async fn pull_and_store_bureau_report(
    pool: &PgPool,
    application_id: Uuid,
    user_id: Uuid,
) -> Result<BureauSummary, String> {
    // 1. Fetch PAN / name from user profile for bureau pull
    let pan_row = sqlx::query_as::<_, (Option<String>, Option<String>, Option<String>)>(
        "SELECT u.first_name, u.last_name, k.govt_id_document_url
         FROM users u
         LEFT JOIN kyc_submissions k ON k.user_id = u.id
         WHERE u.id = $1
         LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("DB error fetching user: {}", e))?;

    let (first_name, _last_name, _govt_id) = pan_row.unwrap_or((None, None, None));

    // 2. Call bureau provider (stub — wire real provider here)
    let summary = fetch_bureau_report_from_provider(
        &first_name.unwrap_or_default(),
        application_id,
    )
    .await;

    // 3. Store bureau_summary on zero_deposits
    let summary_json = serde_json::to_value(&summary).map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE zero_deposits SET bureau_summary = $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(&summary_json)
    .bind(application_id)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error storing bureau: {}", e))?;

    // 4. Queue AI assessment job (Issue 112)
    let _ = queue_ai_assessment_job(pool, application_id).await;

    Ok(summary)
}

/// Pull bureau report for a co-applicant (Issue 133 dependency).
pub async fn pull_bureau_for_co_applicant(
    pool: &PgPool,
    co_applicant_id: Uuid,
    name: &str,
) -> Result<BureauSummary, String> {
    let summary = fetch_bureau_report_from_provider(name, co_applicant_id).await;

    let summary_json = serde_json::to_value(&summary).map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE zd_co_applicants SET bureau_summary = $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(&summary_json)
    .bind(co_applicant_id)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error storing co-applicant bureau: {}", e))?;

    Ok(summary)
}

/// STUB — replace with actual bureau API call (Perfios / CRIF / Experian / CIBIL).
/// Decision on provider is pending (Issue 111).
async fn fetch_bureau_report_from_provider(
    _name: &str,
    _reference: Uuid,
) -> BureauSummary {
    // TODO: Integrate real bureau provider
    // Expected payload shape from provider:
    // { score, dpd_12m, dpd_24m, enquiries_3m, emi_total, card_dues,
    //   write_offs, settlements, suit_filed, reference_id }
    BureauSummary {
        score: None,
        dpd_12m: None,
        dpd_24m: None,
        enquiries_3m: None,
        emi_total_monthly: None,
        card_dues: None,
        write_offs: None,
        settlements: None,
        suit_filed: false,
        provider: "stub".into(),
        reference_id: None,
    }
}

// ---------------------------------------------------------------------------
// AI Assessment client (Issue 112)
// ---------------------------------------------------------------------------

const MAX_RETRIES: u32 = 3;
const BACKOFF_BASE_SECS: u64 = 2;

#[derive(Debug, Serialize)]
struct AssessmentRequest {
    application_id: String,
    kyc_signed_urls: Vec<String>,
    bureau_summary: Value,
    signals: AssessmentSignals,
}

#[derive(Debug, Serialize)]
struct AssessmentSignals {
    device_id: Option<String>,
    ip: Option<String>,
    applications_from_device_30d: i64,
}

/// Queue the AI assessment job for an application.
/// Writes an assessment row with status='pending', then calls the AI service.
/// On failure (3 retries) → sets ai_manual_review with reason AI_UNAVAILABLE.
pub async fn queue_ai_assessment_job(
    pool: &PgPool,
    application_id: Uuid,
) -> Result<String, String> {
    // Fetch config
    let (base_url, timeout_secs) = load_ai_config(pool).await?;

    // Build 15-minute signed S3 URLs for documents
    let doc_urls = fetch_document_signed_urls(pool, application_id).await;

    // Fetch bureau summary
    let bureau_summary = sqlx::query_scalar::<_, Value>(
        "SELECT bureau_summary FROM zero_deposits WHERE id = $1",
    )
    .bind(application_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .unwrap_or(json!({}));

    // Fetch device_id and ip from fraud velocity
    let (device_id, client_ip, device_apps_30d) =
        fetch_device_signals(pool, application_id).await;

    let req_body = AssessmentRequest {
        application_id: application_id.to_string(),
        kyc_signed_urls: doc_urls,
        bureau_summary,
        signals: AssessmentSignals {
            device_id: device_id.clone(),
            ip: client_ip.clone(),
            applications_from_device_30d: device_apps_30d,
        },
    };

    // Try calling AI service with retries
    let client = Client::builder()
        .timeout(Duration::from_secs(timeout_secs as u64))
        .build()
        .map_err(|e| e.to_string())?;

    let assessment_id = format!("pending-{}", uuid::Uuid::new_v4());

    // Insert pending assessment row
    sqlx::query(
        r#"INSERT INTO zero_deposit_ai_assessments
               (zero_deposit_id, assessment_id, status)
           VALUES ($1, $2, 'pending')
           ON CONFLICT (assessment_id) DO NOTHING"#,
    )
    .bind(application_id)
    .bind(&assessment_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    let url = format!("{}/v1/assessments", base_url);

    for attempt in 0..MAX_RETRIES {
        if attempt > 0 {
            let backoff = Duration::from_secs(BACKOFF_BASE_SECS.pow(attempt));
            tokio::time::sleep(backoff).await;
        }

        let resp = client
            .post(&url)
            .bearer_auth(ai_service_token())
            .json(&req_body)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().as_u16() == 202 => {
                if let Ok(body) = r.json::<Value>().await {
                    let real_assessment_id = body
                        .get("assessment_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&assessment_id)
                        .to_string();

                    // Update the row with the real assessment_id from the service
                    sqlx::query(
                        "UPDATE zero_deposit_ai_assessments SET assessment_id = $1, updated_at = NOW() WHERE zero_deposit_id = $2 AND assessment_id = $3",
                    )
                    .bind(&real_assessment_id)
                    .bind(application_id)
                    .bind(&assessment_id)
                    .execute(pool)
                    .await
                    .ok();

                    // Transition application to ai_review
                    sqlx::query(
                        "UPDATE zero_deposits SET status = 'ai_review'::zero_deposit_status, updated_at = NOW() WHERE id = $1 AND status = 'applied'::zero_deposit_status",
                    )
                    .bind(application_id)
                    .execute(pool)
                    .await
                    .ok();

                    return Ok(real_assessment_id);
                }
            }
            _ => continue,
        }
    }

    // All retries exhausted → ai_manual_review
    log::error!(
        "AI service unreachable for application {} — setting ai_manual_review",
        application_id
    );

    sqlx::query(
        "UPDATE zero_deposits SET status = 'ai_manual_review'::zero_deposit_status, updated_at = NOW(), pending_actions = array_append(pending_actions, 'AI_UNAVAILABLE') WHERE id = $1",
    )
    .bind(application_id)
    .execute(pool)
    .await
    .ok();

    sqlx::query(
        "UPDATE zero_deposit_ai_assessments SET status = 'failed', error_code = 'AI_UNAVAILABLE', error_message = 'AI service unreachable after 3 retries', updated_at = NOW() WHERE zero_deposit_id = $1 AND assessment_id = $2",
    )
    .bind(application_id)
    .bind(&assessment_id)
    .execute(pool)
    .await
    .ok();

    Err("AI_UNAVAILABLE".into())
}

/// Queue a document check job (Issue 114).
/// Sends signed URLs + expected values when a tenant submits a redemption.
pub async fn queue_document_check_job(
    pool: &PgPool,
    redemption_id: Uuid,
    application_id: Uuid,
    tenant_name: &str,
    property_address: &str,
    monthly_rent: f64,
    tokens_requested: f64,
    document_urls: Vec<String>,
) -> Result<String, String> {
    let (base_url, timeout_secs) = load_ai_config(pool).await?;

    let client = Client::builder()
        .timeout(Duration::from_secs(timeout_secs as u64))
        .build()
        .map_err(|e| e.to_string())?;

    let payload = json!({
        "redemption_id": redemption_id,
        "application_id": application_id,
        "document_urls": document_urls,
        "expected": {
            "tenant_name": tenant_name,
            "property_address": property_address,
            "monthly_rent": monthly_rent,
            "tokens_requested": tokens_requested
        }
    });

    let url = format!("{}/v1/document-checks", base_url);

    let resp = client
        .post(&url)
        .bearer_auth(ai_service_token())
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Doc check request failed: {}", e))?;

    if resp.status().as_u16() == 202 {
        let body = resp
            .json::<Value>()
            .await
            .map_err(|e| e.to_string())?;
        let check_id = body
            .get("check_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        // Store check_id on redemption
        sqlx::query(
            "UPDATE zd_redemptions SET doc_check_id = $1, status = 'doc_check_pending'::zd_redemption_status, updated_at = NOW() WHERE id = $2",
        )
        .bind(&check_id)
        .bind(redemption_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        Ok(check_id)
    } else {
        Err(format!(
            "AI doc-check service returned HTTP {}",
            resp.status()
        ))
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

async fn load_ai_config(pool: &PgPool) -> Result<(String, i32), String> {
    let row = sqlx::query_as::<_, (String, i32)>(
        "SELECT ai_service_base_url, ai_service_timeout_secs FROM zd_ai_config LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.unwrap_or(("http://localhost:8001".into(), 30)))
}

async fn fetch_document_signed_urls(pool: &PgPool, application_id: Uuid) -> Vec<String> {
    // Fetch document_urls JSONB and itr_document_url, bank_statement_url
    let row = sqlx::query(
        "SELECT document_urls, itr_document_url, bank_statement_url FROM zero_deposits WHERE id = $1",
    )
    .bind(application_id)
    .fetch_optional(pool)
    .await;

    let mut urls: Vec<String> = Vec::new();
    if let Ok(Some(row)) = row {
        // In production: generate 15-minute S3 presigned URLs
        // For now return stored URLs as-is
        if let Ok(doc_urls) = row.try_get::<Value, _>("document_urls") {
            if let Some(arr) = doc_urls.as_object() {
                for (_k, v) in arr {
                    if let Some(s) = v.as_str() {
                        urls.push(s.to_string());
                    }
                }
            }
        }
        if let Ok(Some(itr)) = row.try_get::<Option<String>, _>("itr_document_url") {
            urls.push(itr);
        }
        if let Ok(Some(bank)) = row.try_get::<Option<String>, _>("bank_statement_url") {
            urls.push(bank);
        }
    }

    urls
}

async fn fetch_device_signals(
    pool: &PgPool,
    application_id: Uuid,
) -> (Option<String>, Option<String>, i64) {
    let row = sqlx::query(
        r#"SELECT device_id, client_ip,
           (SELECT COUNT(*) FROM fraud_apply_events fae2
            WHERE fae2.device_id = fae.device_id
              AND fae2.applied_at > NOW() - INTERVAL '30 days') as device_apps_30d
           FROM fraud_apply_events fae
           WHERE application_id = $1
           LIMIT 1"#,
    )
    .bind(application_id)
    .fetch_optional(pool)
    .await;

    match row {
        Ok(Some(r)) => {
            let device_id: Option<String> = r.try_get("device_id").ok().flatten();
            let client_ip: Option<String> = r.try_get("client_ip").ok().flatten();
            let count: i64 = r.try_get("device_apps_30d").unwrap_or(0);
            (device_id, client_ip, count)
        }
        _ => (None, None, 0),
    }
}

fn ai_service_token() -> String {
    std::env::var("ZD_AI_SERVICE_TOKEN").unwrap_or_else(|_| "dev-token".into())
}
