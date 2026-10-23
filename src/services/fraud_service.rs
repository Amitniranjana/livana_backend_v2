/// Issue 96 — Cross-application anomaly / fraud-signal detection
/// Issue 97 — Velocity checks (device / IP sliding-window throttling)
///
/// ## Design decisions
///
/// ### Issue 96 — async, non-blocking
/// Fraud detection runs as a `tokio::spawn` background task after the apply
/// response has already been returned.  A cross-table scan over all existing
/// applications for matching device_fingerprint / bank_account_hash can touch
/// thousands of rows; making it synchronous would add unacceptable latency for
/// legitimate applicants.  Results are stored as a JSONB `fraud_signals` array
/// on `zero_deposits` and surfaced to admins via
/// `GET /api/admin/loans/fraud-signals`.
///
/// ### Issue 97 — middleware hook, Redis-primary / PG-fallback
/// The middleware checks two independent counters before the apply handler runs:
///   • `device_id` — at most N applications per window from one device
///   • `ip_address` — at most M applications per window from one IP
/// Redis is the fast path (INCR + EXPIRE, O(1)).  If Redis is absent the
/// middleware falls back to a COUNT(*) query on `loan_apply_events`
/// (correct but slower — acceptable for low-traffic fallback mode).

use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;
use serde_json::{json, Value};

// ─── Velocity limits ────────────────────────────────────────────────────────

/// Maximum loan applications per device within DEVICE_WINDOW_SECS.
pub const DEVICE_LIMIT: i64 = 3;
/// Maximum loan applications per IP within IP_WINDOW_SECS.
pub const IP_LIMIT: i64 = 5;
/// Sliding-window duration for device-based throttle (seconds).
pub const DEVICE_WINDOW_SECS: i64 = 3600; // 1 hour
/// Sliding-window duration for IP-based throttle (seconds).
pub const IP_WINDOW_SECS: i64 = 3600; // 1 hour

// ─── Fraud-signal codes ─────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FraudSignal {
    /// Machine-readable code, e.g. "SHARED_DEVICE"
    pub code: String,
    /// Short human label for the admin UI
    pub label: String,
    /// Full detail explaining why this was flagged
    pub detail: String,
    /// Severity: "LOW" | "MEDIUM" | "HIGH"
    pub severity: String,
}

// ─── Issue 97: Velocity check (called from middleware) ──────────────────────

/// Checks whether a device or IP has exceeded the apply velocity limit using
/// Redis as the primary counter.  Falls back to a Postgres COUNT query when
/// Redis is unavailable.
///
/// Returns `Some(reason_message)` if the request should be rejected,
/// `None` if it is within limits.
pub async fn check_velocity(
    db: &PgPool,
    redis_pool: &Option<redis::aio::ConnectionManager>,
    device_id: Option<&str>,
    ip_address: Option<&str>,
) -> Option<String> {
    // ── Redis fast path ──────────────────────────────────────────────────
    if let Some(mut redis) = redis_pool.clone() {
        use redis::AsyncCommands;

        if let Some(device) = device_id {
            let key = format!("velocity:device:{}", device);
            let count: i64 = redis.incr(&key, 1).await.unwrap_or(1);
            if count == 1 {
                // First hit — set TTL
                let _: () = redis.expire(&key, DEVICE_WINDOW_SECS).await.unwrap_or(());
            }
            if count > DEVICE_LIMIT {
                return Some(format!(
                    "Too many loan applications from this device. Please wait before trying again."
                ));
            }
        }

        if let Some(ip) = ip_address {
            let key = format!("velocity:ip:{}", ip);
            let count: i64 = redis.incr(&key, 1).await.unwrap_or(1);
            if count == 1 {
                let _: () = redis.expire(&key, IP_WINDOW_SECS).await.unwrap_or(());
            }
            if count > IP_LIMIT {
                return Some(format!(
                    "Too many loan applications from this network. Please try again later."
                ));
            }
        }

        return None; // Within limits
    }

    // ── Postgres fallback (no Redis) ─────────────────────────────────────
    let window_start = Utc::now() - chrono::Duration::seconds(DEVICE_WINDOW_SECS);

    if let Some(device) = device_id {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM loan_apply_events
            WHERE device_id = $1
              AND created_at >= $2
            "#,
        )
        .bind(device)
        .bind(window_start)
        .fetch_one(db)
        .await
        .unwrap_or(0);

        if count >= DEVICE_LIMIT {
            return Some(
                "Too many loan applications from this device. Please wait before trying again."
                    .to_string(),
            );
        }
    }

    if let Some(ip) = ip_address {
        let ip_window_start = Utc::now() - chrono::Duration::seconds(IP_WINDOW_SECS);
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM loan_apply_events
            WHERE ip_address = $1::INET
              AND created_at >= $2
            "#,
        )
        .bind(ip)
        .bind(ip_window_start)
        .fetch_one(db)
        .await
        .unwrap_or(0);

        if count >= IP_LIMIT {
            return Some(
                "Too many loan applications from this network. Please try again later."
                    .to_string(),
            );
        }
    }

    None
}

/// Persist one apply-event row (called after the application is successfully
/// inserted so the event is only logged for non-rejected requests).
pub async fn record_apply_event(
    db: &PgPool,
    user_id: Uuid,
    zero_deposit_id: Uuid,
    device_id: Option<&str>,
    ip_address: Option<&str>,
) {
    let _ = sqlx::query(
        r#"
        INSERT INTO loan_apply_events (id, device_id, ip_address, user_id, zero_deposit_id)
        VALUES (gen_random_uuid(), $1, $2::INET, $3, $4)
        "#,
    )
    .bind(device_id)
    .bind(ip_address)
    .bind(user_id)
    .bind(zero_deposit_id)
    .execute(db)
    .await;
    // Silently ignore errors — velocity tracking must not fail the happy path.
}

// ─── Issue 96: Gemini AI income proof vs stated employer verification ──────

/// Issue 96: AI-assisted verification of stated employer vs income proof / bank statement.
/// Uses Gemini LLM when GEMINI_API_KEY is configured in .env.
async fn verify_employer_income_with_gemini(
    applicant_name: &str,
    stated_employer: &str,
    monthly_income: f64,
    monthly_rent: f64,
    bank_statement_url: Option<&str>,
    itr_document_url: Option<&str>,
    bank_flags: &[String],
    salary_credits: Option<i64>,
) -> Option<FraudSignal> {
    let api_key = std::env::var("GEMINI_API_KEY").ok()?;
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return None;
    }

    let model = std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-1.5-flash".to_string());
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );

    let prompt = format!(
        r#"You are an expert financial fraud detection and underwriting engine for digital loan/rental-deposit applications in India.
Analyze if the applicant's declared income and stated employer are consistent with the provided financial proof and transaction indicators.

Application Profile:
- Applicant Name: {}
- Stated Employer / Organization: {}
- Declared Monthly Income: INR {:.2}
- Declared Monthly Rent: INR {:.2}
- Bank Statement Document URL: {}
- ITR Document URL: {}
- Detected Bank Statement Risk Flags: {:?}
- Detected Regular Salary Credit Count: {:?}

Evaluation Guidelines:
1. Check if the declared monthly income (INR {:.2}) is plausible for an employee at "{}" in India.
2. If regular salary credits are 0 or bank flags contain "IRREGULAR_INCOME" / "BOUNCED_CHEQUE" while claiming full-time corporate employment, flag as inconsistent.
3. If no income proof document (bank statement or ITR) was provided, or if the income appears heavily inflated or incompatible with the employer profile, flag as inconsistent.
4. If everything looks plausible and consistent, return "is_inconsistent": false.

Respond ONLY with valid JSON in exactly this schema:
{{
  "is_inconsistent": <true|false>,
  "severity": <"LOW"|"MEDIUM"|"HIGH">,
  "reason": "<Detailed factual explanation for fraud review>"
}}"#,
        applicant_name,
        stated_employer,
        monthly_income,
        monthly_rent,
        bank_statement_url.unwrap_or("None provided"),
        itr_document_url.unwrap_or("None provided"),
        bank_flags,
        salary_credits,
        monthly_income,
        stated_employer,
    );

    let payload = json!({
        "contents": [{
            "parts": [{ "text": prompt }]
        }],
        "generationConfig": {
            "temperature": 0.1,
            "responseMimeType": "application/json"
        }
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .ok()?;

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        log::warn!("[gemini] API error status: {}", response.status());
        return None;
    }

    let json_resp: Value = response.json().await.ok()?;
    let text = json_resp
        .pointer("/candidates/0/content/parts/0/text")
        .and_then(|v| v.as_str())?;

    let parsed: Value = serde_json::from_str(text.trim()).ok()?;
    let is_inconsistent = parsed.get("is_inconsistent").and_then(|v| v.as_bool()).unwrap_or(false);

    if is_inconsistent {
        let severity = parsed
            .get("severity")
            .and_then(|v| v.as_str())
            .unwrap_or("HIGH")
            .to_string();
        let reason = parsed
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("Income proof or transaction history inconsistent with stated employer.")
            .to_string();

        Some(FraudSignal {
            code: "INCOME_EMPLOYER_INCONSISTENT".to_string(),
            label: "Income proof inconsistent with stated employer".to_string(),
            detail: reason,
            severity,
        })
    } else {
        None
    }
}

// ─── Issue 96: Fraud-signal detection (called from tokio::spawn) ─────────────

/// Run all fraud-signal checks for a newly submitted zero_deposit application.
///
/// Checks performed:
///   1. Same device_fingerprint used by multiple distinct user identities
///   2. Same bank_account_hash used by multiple distinct user identities
///   3. Stated income & employer inconsistent with income proof (AI Gemini analysis + heuristic fallback)
///   4. Stated income < 1.5× monthly rent (debt service stress)
///   5. Rapid re-application by same user (> 2 applications in 24 h)
///
/// Results are written back to `zero_deposits.fraud_signals`.
pub async fn detect_fraud_signals(
    db: &PgPool,
    zero_deposit_id: Uuid,
) {
    // Load the application row joined with user, KYC, and recent bank statement analysis
    let row = sqlx::query_scalar::<_, Value>(
        r#"
        SELECT row_to_json(t) FROM (
            SELECT zd.id, zd.user_id,
                   zd.device_fingerprint, zd.bank_account_hash,
                   zd.monthly_income::FLOAT8, zd.monthly_rent::FLOAT8,
                   zd.bank_statement_url, zd.itr_document_url,
                   COALESCE(zd.stated_employer, ks.company_name) AS stated_employer,
                   ks.extracted_income::FLOAT8 AS kyc_extracted_income,
                   u.full_name, u.email, u.phone_no,
                   (
                       SELECT bsa.flags
                       FROM bank_statement_analyses bsa
                       WHERE bsa.zero_deposit_id = zd.id
                       ORDER BY bsa.created_at DESC
                       LIMIT 1
                   ) AS bank_flags,
                   (
                       SELECT bsa.salary_credit_count
                       FROM bank_statement_analyses bsa
                       WHERE bsa.zero_deposit_id = zd.id
                       ORDER BY bsa.created_at DESC
                       LIMIT 1
                   ) AS salary_credit_count
            FROM   zero_deposits zd
            JOIN   users u ON u.id = zd.user_id
            LEFT JOIN kyc_submissions ks ON ks.id = zd.kyc_id
            WHERE  zd.id = $1
        ) t
        "#,
    )
    .bind(zero_deposit_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    let Some(row) = row else { return };

    let mut signals: Vec<FraudSignal> = Vec::new();

    let user_id = row.get("user_id").and_then(|v| v.as_str()).unwrap_or("");
    let applicant_name = row.get("full_name").and_then(|v| v.as_str()).unwrap_or("Applicant");
    let stated_employer = row.get("stated_employer").and_then(|v| v.as_str()).unwrap_or("").trim();
    let device_fp = row.get("device_fingerprint").and_then(|v| v.as_str());
    let bank_hash = row.get("bank_account_hash").and_then(|v| v.as_str());
    let bank_statement_url = row.get("bank_statement_url").and_then(|v| v.as_str());
    let itr_document_url = row.get("itr_document_url").and_then(|v| v.as_str());
    let monthly_income = row.get("monthly_income").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let monthly_rent = row.get("monthly_rent").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let salary_credit_count = row.get("salary_credit_count").and_then(|v| v.as_i64());

    let bank_flags: Vec<String> = row
        .get("bank_flags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    // ── Check 1: shared device_fingerprint across different users ─────────
    if let Some(fp) = device_fp {
        if !fp.is_empty() {
            let conflict_count: i64 = sqlx::query_scalar(
                r#"
                SELECT COUNT(DISTINCT user_id) FROM zero_deposits
                WHERE  device_fingerprint = $1
                  AND  user_id::TEXT <> $2
                  AND  device_fingerprint IS NOT NULL
                "#,
            )
            .bind(fp)
            .bind(user_id)
            .fetch_one(db)
            .await
            .unwrap_or(0);

            if conflict_count > 0 {
                signals.push(FraudSignal {
                    code: "SHARED_DEVICE".to_string(),
                    label: "Device shared across multiple applicants".to_string(),
                    detail: format!(
                        "The device fingerprint used for this application matches {} other applicant identit{}. \
                         Possible synthetic identity or account takeover.",
                        conflict_count,
                        if conflict_count == 1 { "y" } else { "ies" }
                    ),
                    severity: if conflict_count >= 3 { "HIGH".to_string() } else { "MEDIUM".to_string() },
                });
            }
        }
    }

    // ── Check 2: shared bank account hash across different users ──────────
    if let Some(hash) = bank_hash {
        if !hash.is_empty() {
            let conflict_count: i64 = sqlx::query_scalar(
                r#"
                SELECT COUNT(DISTINCT user_id) FROM zero_deposits
                WHERE  bank_account_hash = $1
                  AND  user_id::TEXT <> $2
                  AND  bank_account_hash IS NOT NULL
                "#,
            )
            .bind(hash)
            .bind(user_id)
            .fetch_one(db)
            .await
            .unwrap_or(0);

            if conflict_count > 0 {
                signals.push(FraudSignal {
                    code: "SHARED_BANK_ACCOUNT".to_string(),
                    label: "Bank account linked to multiple applicant identities".to_string(),
                    detail: format!(
                        "The bank account (normalised hash) is also associated with {} other applicant identit{}. \
                         This is a strong indicator of a synthetic identity ring or mule account.",
                        conflict_count,
                        if conflict_count == 1 { "y" } else { "ies" }
                    ),
                    severity: "HIGH".to_string(),
                });
            }
        }
    }

    // ── Check 3: income proof inconsistent with stated employer (AI Gemini + heuristic) ──
    let gemini_signal = verify_employer_income_with_gemini(
        applicant_name,
        stated_employer,
        monthly_income,
        monthly_rent,
        bank_statement_url,
        itr_document_url,
        &bank_flags,
        salary_credit_count,
    )
    .await;

    if let Some(signal) = gemini_signal {
        signals.push(signal);
    } else {
        // Fallback rule-based check when Gemini is not configured or offline:
        if !stated_employer.is_empty() {
            if bank_statement_url.is_none() && itr_document_url.is_none() {
                signals.push(FraudSignal {
                    code: "INCOME_PROOF_MISSING".to_string(),
                    label: "No income proof provided for stated employer".to_string(),
                    detail: format!(
                        "Applicant declared employment at '{}' with monthly income {:.0} INR, but attached no bank statement or ITR document.",
                        stated_employer, monthly_income
                    ),
                    severity: "MEDIUM".to_string(),
                });
            } else if bank_flags.iter().any(|f| f == "IRREGULAR_INCOME") && salary_credit_count.unwrap_or(0) == 0 {
                signals.push(FraudSignal {
                    code: "INCOME_EMPLOYER_INCONSISTENT".to_string(),
                    label: "Income proof inconsistent with stated employer".to_string(),
                    detail: format!(
                        "Applicant claims salaried employment at '{}', but bank statement analysis detected irregular cashflow with zero detected salary credits.",
                        stated_employer
                    ),
                    severity: "HIGH".to_string(),
                });
            }
        }
    }

    // ── Check 4: income inconsistent with rent obligation ──────────
    if monthly_income > 0.0 && monthly_rent > 0.0 {
        let ratio = monthly_income / monthly_rent;
        if ratio < 1.5 {
            signals.push(FraudSignal {
                code: "INCOME_RENT_MISMATCH".to_string(),
                label: "Stated income inconsistent with rent obligation".to_string(),
                detail: format!(
                    "Declared monthly income ({:.0} INR) is only {:.1}× the monthly rent ({:.0} INR). \
                     This is below the 1.5× threshold, suggesting the income proof may be inflated \
                     or the applicant cannot realistically service this obligation.",
                    monthly_income, ratio, monthly_rent
                ),
                severity: if ratio < 1.0 { "HIGH".to_string() } else { "MEDIUM".to_string() },
            });
        }
    }

    // ── Check 5: rapid re-application by same user (velocity per user) ────
    let user_uuid = Uuid::parse_str(user_id).ok();
    if let Some(uid) = user_uuid {
        let window_start = Utc::now() - chrono::Duration::hours(24);
        let recent_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM zero_deposits
            WHERE  user_id = $1
              AND  id <> $2
              AND  created_at >= $3
            "#,
        )
        .bind(uid)
        .bind(zero_deposit_id)
        .bind(window_start)
        .fetch_one(db)
        .await
        .unwrap_or(0);

        if recent_count >= 2 {
            signals.push(FraudSignal {
                code: "RAPID_REAPPLICATION".to_string(),
                label: "Multiple applications from same user within 24 h".to_string(),
                detail: format!(
                    "This user submitted {} other application(s) in the last 24 hours. \
                     Legitimate applicants rarely re-apply this quickly; possible credential stuffing \
                     or testing behaviour.",
                    recent_count
                ),
                severity: if recent_count >= 4 { "HIGH".to_string() } else { "MEDIUM".to_string() },
            });
        }
    }

    // ── Write signals back (no signals = empty array, clears old flags) ───
    if signals.is_empty() {
        // Nothing to flag — ensure column stays clean (idempotent)
        let _ = sqlx::query(
            "UPDATE zero_deposits SET fraud_signals = '[]' WHERE id = $1",
        )
        .bind(zero_deposit_id)
        .execute(db)
        .await;
    } else {
        let signals_json = serde_json::to_value(&signals).unwrap_or(json!([]));
        let _ = sqlx::query(
            r#"
            UPDATE zero_deposits
            SET    fraud_signals    = $1,
                   fraud_flagged_at = NOW(),
                   updated_at       = NOW()
            WHERE  id = $2
            "#,
        )
        .bind(&signals_json)
        .bind(zero_deposit_id)
        .execute(db)
        .await;

        log::warn!(
            "[fraud] zero_deposit {} flagged with {} signal(s): {:?}",
            zero_deposit_id,
            signals.len(),
            signals.iter().map(|s| &s.code).collect::<Vec<_>>()
        );
    }
}

// ─── Issue 96: Batch anomaly detection scan ─────────────────────────────────

/// Issue 96: Batch anomaly detection scan across all recent applications (last 90 days).
/// Can be invoked by admin endpoint `POST /api/admin/loans/fraud-signals/scan`
/// or scheduled as a nightly batch job.
pub async fn run_batch_fraud_scan(
    db: &PgPool,
) -> Result<Value, String> {
    let app_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id FROM zero_deposits
        WHERE created_at >= NOW() - INTERVAL '90 days'
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())?;

    let total = app_ids.len();
    let mut flagged = 0;

    for id in app_ids {
        detect_fraud_signals(db, id).await;

        let count: i64 = sqlx::query_scalar(
            "SELECT jsonb_array_length(fraud_signals) FROM zero_deposits WHERE id = $1",
        )
        .bind(id)
        .fetch_one(db)
        .await
        .unwrap_or(0);

        if count > 0 {
            flagged += 1;
        }
    }

    Ok(json!({
        "scanned_applications": total,
        "flagged_applications": flagged,
        "scanned_at": Utc::now()
    }))
}

// ─── Issue 96: Admin fraud-signals list (query helper) ──────────────────────

/// Returns all zero_deposit applications that have at least one fraud signal,
/// joined with applicant identity info.  Used by
/// `GET /api/admin/loans/fraud-signals`.
pub async fn list_fraud_flagged_applications(
    db: &PgPool,
) -> Result<Vec<Value>, String> {
    let rows = sqlx::query_scalar::<_, Value>(
        r#"
        SELECT row_to_json(t) FROM (
            SELECT
                zd.id                       AS zero_deposit_id,
                zd.user_id,
                u.full_name                 AS applicant_name,
                u.email                     AS applicant_email,
                u.phone_no                  AS applicant_phone,
                COALESCE(zd.stated_employer, ks.company_name) AS stated_employer,
                zd.monthly_income::FLOAT8,
                zd.monthly_rent::FLOAT8,
                zd.requested_deposit_amount::FLOAT8,
                zd.status,
                zd.fraud_signals,
                zd.fraud_flagged_at,
                zd.device_fingerprint,
                -- Don't return the raw hash — return a boolean presence flag
                (zd.bank_account_hash IS NOT NULL) AS has_bank_account_hash,
                zd.applicant_ip::TEXT,
                zd.created_at,
                -- Risk score if already computed
                lrs.risk_tier,
                lrs.probability_of_default::FLOAT8
            FROM   zero_deposits zd
            JOIN   users u ON u.id = zd.user_id
            LEFT JOIN kyc_submissions ks ON ks.id = zd.kyc_id
            LEFT JOIN loan_risk_scores lrs ON lrs.zero_deposit_id = zd.id
            WHERE  zd.fraud_flagged_at IS NOT NULL
              AND  jsonb_array_length(zd.fraud_signals) > 0
            ORDER  BY zd.fraud_flagged_at DESC
        ) t
        "#,
    )
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows)
}

/// Returns a summary count of signals grouped by signal code for the admin
/// dashboard overview widget.
pub async fn fraud_signal_summary(
    db: &PgPool,
) -> Result<Value, String> {
    let total_flagged: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM zero_deposits
        WHERE fraud_flagged_at IS NOT NULL
          AND jsonb_array_length(fraud_signals) > 0
        "#,
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    // Count per signal code using jsonb_array_elements
    let by_code = sqlx::query_scalar::<_, Value>(
        r#"
        SELECT row_to_json(t) FROM (
            SELECT
                signal->>'code'     AS code,
                signal->>'severity' AS severity,
                COUNT(*)            AS occurrences
            FROM zero_deposits,
                 jsonb_array_elements(fraud_signals) AS signal
            WHERE fraud_flagged_at IS NOT NULL
            GROUP BY signal->>'code', signal->>'severity'
            ORDER BY occurrences DESC
        ) t
        "#,
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    Ok(json!({
        "total_flagged_applications": total_flagged,
        "by_code": by_code
    }))
}
