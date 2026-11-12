//! Issue 111 — Credit Bureau Pull at Consent
//! Issue 112 — AI Service Client + Redis Queue
//! Issue 114 — AI Document-Check Client
//!
//! Outbound contract: Backend -> AI: POST /v1/assessments
//! Authenticated with service token. S3 signed URLs valid 15 minutes.
//! Outbound client & Bureau client are behind traits so they can be mocked/swapped.

use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::time::Duration;
use uuid::Uuid;

use crate::app_state::AppState;

// ---------------------------------------------------------------------------
// Bureau Types & Trait (Issue 111)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BureauSummary {
    pub score: Option<i32>,
    pub report_date: Option<String>,
    pub dpd_12m: Option<i32>,
    pub dpd_24m: Option<i32>,
    pub enquiries_3m: Option<i32>,
    pub emi_total_monthly: Option<f64>,
    pub card_dues: Option<f64>,
    pub write_offs: Option<i32>,
    pub settlements: Option<i32>,
    pub suit_filed: bool,
    pub provider: String,
    pub reference_id: Option<String>,
}

#[async_trait]
pub trait BureauClient: Send + Sync {
    async fn pull_report(&self, name: &str, pan: &str, app_id: Uuid) -> Result<BureauSummary, String>;
}

pub struct StubBureauClient;

#[async_trait]
impl BureauClient for StubBureauClient {
    async fn pull_report(&self, _name: &str, _pan: &str, _app_id: Uuid) -> Result<BureauSummary, String> {
        // Stub implementation returning a clean credit profile
        Ok(BureauSummary {
            score: Some(735),
            report_date: Some(chrono::Utc::now().date_naive().to_string()),
            dpd_12m: Some(0),
            dpd_24m: Some(0),
            enquiries_3m: Some(1),
            emi_total_monthly: Some(4000.0),
            card_dues: Some(1500.0),
            write_offs: Some(0),
            settlements: Some(0),
            suit_filed: false,
            provider: "stub-cibil".into(),
            reference_id: Some(Uuid::new_v4().to_string()),
        })
    }
}

// ---------------------------------------------------------------------------
// AI Client Types & Trait (Issue 112)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiAssessmentOutboundRequest {
    pub assessment_id: Uuid,
    pub zero_deposit_id: Uuid,
    pub callback_url: String,
    pub application: Value,
    pub kyc: Value,
    pub documents: Value,
    pub bureau: Value,
    pub signals: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiAssessmentOutboundResponse {
    pub assessment_id: Uuid,
}

#[async_trait]
pub trait AiClient: Send + Sync {
    async fn trigger_assessment(
        &self,
        endpoint: &str,
        service_token: &str,
        req: &AiAssessmentOutboundRequest,
        timeout_secs: u64,
    ) -> Result<AiAssessmentOutboundResponse, String>;
}

pub struct HttpAiClient;

#[async_trait]
impl AiClient for HttpAiClient {
    async fn trigger_assessment(
        &self,
        endpoint: &str,
        service_token: &str,
        req: &AiAssessmentOutboundRequest,
        timeout_secs: u64,
    ) -> Result<AiAssessmentOutboundResponse, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| e.to_string())?;

        let url = format!("{}/v1/assessments", endpoint.trim_end_matches('/'));

        let resp = client
            .post(&url)
            .bearer_auth(service_token)
            .json(req)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if resp.status().as_u16() == 202 || resp.status().is_success() {
            let body: Value = resp.json().await.unwrap_or(json!({}));
            let aid_str = body.get("assessment_id").and_then(|v| v.as_str());
            let aid = aid_str
                .and_then(|s| Uuid::parse_str(s).ok())
                .unwrap_or(req.assessment_id);

            Ok(AiAssessmentOutboundResponse { assessment_id: aid })
        } else {
            Err(format!("AI service responded with HTTP {}", resp.status()))
        }
    }
}

// ---------------------------------------------------------------------------
// Queueing & Orchestration
// ---------------------------------------------------------------------------

const MAX_RETRIES: u32 = 3;
const BACKOFF_BASE_SECS: u64 = 2;

pub async fn pull_and_store_bureau_report(
    pool: &PgPool,
    application_id: Uuid,
    user_id: Uuid,
) -> Result<BureauSummary, String> {
    pull_and_store_bureau_report_with_client(pool, &StubBureauClient, application_id, user_id).await
}

pub async fn pull_and_store_bureau_report_with_client(
    pool: &PgPool,
    client: &dyn BureauClient,
    application_id: Uuid,
    user_id: Uuid,
) -> Result<BureauSummary, String> {
    let pan_row = sqlx::query_as::<_, (Option<String>, Option<String>, Option<String>)>(
        r#"SELECT u.first_name, u.last_name, k.govt_id_number
           FROM users u
           LEFT JOIN kyc_submissions k ON k.user_id = u.id
           WHERE u.id = $1
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let (first_name, _last_name, govt_id) = pan_row.unwrap_or((None, None, None));
    let name = first_name.unwrap_or_else(|| "Tenant".into());
    let pan = govt_id.unwrap_or_else(|| "ABCDE1234F".into());

    let summary = client.pull_report(&name, &pan, application_id).await?;

    let summary_json = serde_json::to_value(&summary).map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE zero_deposits SET bureau_summary = $1, updated_at = NOW() WHERE id = $2"
    )
    .bind(&summary_json)
    .bind(application_id)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    Ok(summary)
}

pub async fn queue_ai_assessment_job(
    pool: &PgPool,
    redis_pool: Option<redis::aio::ConnectionManager>,
    application_id: Uuid,
    user_id: Uuid,
) -> Result<Uuid, String> {
    let assessment_id = Uuid::new_v4();

    // 1. Mark previous assessments for this application as is_current = false
    sqlx::query(
        "UPDATE zero_deposit_ai_assessments SET is_current = false WHERE zero_deposit_id = $1"
    )
    .bind(application_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    // 2. Insert queued assessment row
    sqlx::query(
        r#"INSERT INTO zero_deposit_ai_assessments
               (id, assessment_id, zero_deposit_id, status, is_current, attempts, created_at)
           VALUES ($1, $1::TEXT, $2, 'queued', true, 0, NOW())"#
    )
    .bind(assessment_id)
    .bind(application_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    // 3. Pull bureau summary if not present
    let bureau_exists: bool = sqlx::query_scalar(
        "SELECT bureau_summary IS NOT NULL AND bureau_summary != '{}'::jsonb FROM zero_deposits WHERE id = $1"
    )
    .bind(application_id)
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if !bureau_exists {
        let _ = pull_and_store_bureau_report(pool, application_id, user_id).await;
    }

    // 4. If Redis available, push to queue
    if let Some(mut r) = redis_pool {
        use redis::AsyncCommands;
        let job_payload = json!({
            "assessment_id": assessment_id,
            "zero_deposit_id": application_id,
            "created_at": Utc::now().to_rfc3339()
        });
        let _: Result<(), _> = r.lpush("zd:assessment:queue", job_payload.to_string()).await;
    }

    // 5. Spawn async worker task to execute outbound AI call with retries
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        execute_ai_assessment_outbound(&pool_clone, &HttpAiClient, assessment_id, application_id).await;
    });

    Ok(assessment_id)
}

pub async fn execute_ai_assessment_outbound(
    pool: &PgPool,
    client: &dyn AiClient,
    assessment_id: Uuid,
    application_id: Uuid,
) {
    let (endpoint, token, timeout_secs, callback_base) = load_ai_service_config(pool).await;

    // Load application payload
    let app_row = sqlx::query(
        r#"SELECT
            z.id, z.user_id, z.monthly_rent::FLOAT8 as rent,
            COALESCE(z.monthly_income::FLOAT8, 0.0) as declared_income,
            COALESCE(z.variable_pay_monthly::FLOAT8, 0.0) as var_pay,
            COALESCE(z.employment_type, 'salaried') as emp_type,
            z.employer_name,
            z.bureau_summary,
            z.document_urls,
            z.applicant_ip,
            z.device_fingerprint,
            k.full_name,
            k.date_of_birth
        FROM zero_deposits z
        LEFT JOIN kyc_submissions k ON k.id = z.kyc_id
        WHERE z.id = $1"#
    )
    .bind(application_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let app_row = match app_row {
        Some(r) => r,
        None => return,
    };

    let rent: f64 = app_row.get("rent");
    let declared_income: f64 = app_row.get("declared_income");
    let var_pay: f64 = app_row.get("var_pay");
    let emp_type: String = app_row.get("emp_type");
    let employer_name: Option<String> = app_row.try_get("employer_name").ok();
    let bureau: Value = app_row.try_get("bureau_summary").unwrap_or(json!({}));
    let documents: Value = app_row.try_get("document_urls").unwrap_or(json!({}));
    let client_ip: Option<String> = app_row.try_get::<Option<String>, _>("applicant_ip").ok().flatten();
    let device_id: Option<String> = app_row.try_get::<Option<String>, _>("device_fingerprint").ok().flatten();
    let full_name: Option<String> = app_row.try_get("full_name").ok();
    let dob: Option<String> = app_row.try_get("date_of_birth").ok();

    let callback_url = format!("{}/internal/zero-deposit/assessments/{}/result", callback_base.trim_end_matches('/'), assessment_id);

    let outbound_req = AiAssessmentOutboundRequest {
        assessment_id,
        zero_deposit_id: application_id,
        callback_url,
        application: json!({
            "monthly_rent": rent,
            "employment_type": emp_type,
            "employer_name": employer_name.unwrap_or_default(),
            "declared_monthly_income": declared_income,
            "variable_pay_monthly": var_pay
        }),
        kyc: json!({
            "name_on_id": full_name.unwrap_or_default(),
            "dob": dob.unwrap_or_default(),
            "ocr_text": ""
        }),
        documents,
        bureau,
        signals: json!({
            "device_id": device_id.unwrap_or_default(),
            "ip": client_ip.unwrap_or_default(),
            "applications_from_device_30d": 1
        }),
    };

    // Retry loop with exponential backoff (2s, 4s, 8s)
    let mut success = false;
    for attempt in 0..MAX_RETRIES {
        if attempt > 0 {
            let backoff = Duration::from_secs(BACKOFF_BASE_SECS.pow(attempt));
            tokio::time::sleep(backoff).await;
        }

        // Increment attempts on assessment row
        let _ = sqlx::query(
            "UPDATE zero_deposit_ai_assessments SET attempts = attempts + 1 WHERE id = $1"
        )
        .bind(assessment_id)
        .execute(pool)
        .await;

        match client.trigger_assessment(&endpoint, &token, &outbound_req, timeout_secs).await {
            Ok(_) => {
                success = true;
                break;
            }
            Err(e) => {
                log::warn!("AI service attempt {} failed: {}", attempt + 1, e);
            }
        }
    }

    if !success {
        // Mark assessment as failed and transition application to ai_manual_review with AI_UNAVAILABLE
        let _ = sqlx::query(
            r#"UPDATE zero_deposit_ai_assessments
               SET status = 'failed', error_code = 'AI_UNAVAILABLE', completed_at = NOW()
               WHERE id = $1"#
        )
        .bind(assessment_id)
        .execute(pool)
        .await;

        let _ = sqlx::query(
            r#"UPDATE zero_deposits
               SET status = 'ai_manual_review'::zero_deposit_status, updated_at = NOW()
               WHERE id = $1 AND status = 'ai_review'::zero_deposit_status"#
        )
        .bind(application_id)
        .execute(pool)
        .await;
    }
}

async fn load_ai_service_config(_pool: &PgPool) -> (String, String, u64, String) {
    let endpoint = std::env::var("ZD_AI_SERVICE_URL")
        .unwrap_or_else(|_| "https://ai.livana.eco".to_string());
    let token = std::env::var("ZD_AI_SERVICE_TOKEN")
        .unwrap_or_else(|_| "dev-ai-token".to_string());
    let callback_base = std::env::var("API_BASE_URL")
        .unwrap_or_else(|_| "https://api.livana.eco".to_string());
    let timeout_secs = 30u64;

    (endpoint, token, timeout_secs, callback_base)
}

// ---------------------------------------------------------------------------
// Issue 114: Outbound AI Document Check
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDocumentCheckOutboundRequest {
    pub check_id: Uuid,
    pub redemption_id: Uuid,
    pub zero_deposit_id: Uuid,
    pub callback_url: String,
    pub rental_agreement_url: Option<String>,
    pub landlord_bank_proof_url: Option<String>,
    pub landlord_name: Option<String>,
    pub declared_monthly_rent: Option<f64>,
    pub property_address: Option<String>,
}

pub async fn queue_ai_document_check_job(
    state: &AppState,
    redemption_id: Uuid,
    zero_deposit_id: Uuid,
) {
    let check_id = Uuid::new_v4();
    let state_clone = state.clone();

    tokio::spawn(async move {
        execute_ai_document_check_outbound(&state_clone, check_id, redemption_id, zero_deposit_id).await;
    });
}

pub async fn execute_ai_document_check_outbound(
    state: &AppState,
    check_id: Uuid,
    redemption_id: Uuid,
    zero_deposit_id: Uuid,
) {
    let (endpoint, token, timeout_secs, callback_base) = load_ai_service_config(&state.db).await;

    // 1. Fetch details
    let red_row = sqlx::query(
        "SELECT property_address, monthly_rent::FLOAT8 as rent, landlord_name FROM zd_redemptions WHERE id = $1"
    )
    .bind(redemption_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (prop_addr, rent, landlord_name) = match red_row {
        Some(r) => (
            r.try_get::<String, _>("property_address").ok(),
            r.try_get::<f64, _>("rent").ok(),
            r.try_get::<String, _>("landlord_name").ok(),
        ),
        None => (None, None, None),
    };

    // 2. Fetch doc urls
    let agreement_s3: Option<String> = sqlx::query_scalar(
        "SELECT url FROM zd_redemption_documents WHERE redemption_id = $1 AND doc_type IN ('agreement', 'rental_agreement') AND url IS NOT NULL ORDER BY version DESC LIMIT 1"
    )
    .bind(redemption_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let bank_proof_s3: Option<String> = sqlx::query_scalar(
        "SELECT url FROM zd_redemption_documents WHERE redemption_id = $1 AND doc_type IN ('bank_proof', 'landlord_bank_proof') AND url IS NOT NULL ORDER BY version DESC LIMIT 1"
    )
    .bind(redemption_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let agreement_url = agreement_s3.as_deref().map(|k| crate::services::zd_redemption_service::generate_signed_url(k, &state.storage_service));
    let bank_proof_url = bank_proof_s3.as_deref().map(|k| crate::services::zd_redemption_service::generate_signed_url(k, &state.storage_service));

    let callback_url = format!("{}/internal/zero-deposit/document-checks/{}/result", callback_base.trim_end_matches('/'), check_id);

    let outbound_req = AiDocumentCheckOutboundRequest {
        check_id,
        redemption_id,
        zero_deposit_id,
        callback_url,
        rental_agreement_url: agreement_url,
        landlord_bank_proof_url: bank_proof_url,
        landlord_name,
        declared_monthly_rent: rent,
        property_address: prop_addr,
    };

    // Record check_id in zd_redemptions
    let _ = sqlx::query("UPDATE zd_redemptions SET doc_check_id = $1 WHERE id = $2")
        .bind(check_id.to_string())
        .bind(redemption_id)
        .execute(&state.db)
        .await;

    // Execute HTTP call with retry
    let client = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .unwrap_or_default();

    let url = format!("{}/v1/document-checks", endpoint.trim_end_matches('/'));

    for attempt in 0..MAX_RETRIES {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(BACKOFF_BASE_SECS.pow(attempt))).await;
        }

        match client
            .post(&url)
            .bearer_auth(&token)
            .json(&outbound_req)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 202 => {
                break;
            }
            Ok(resp) => {
                log::warn!("AI doc-check responded with status {}", resp.status());
            }
            Err(e) => {
                log::warn!("AI doc-check outbound attempt {} failed: {}", attempt + 1, e);
            }
        }
    }
}
