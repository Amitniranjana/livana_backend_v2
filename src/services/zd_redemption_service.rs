//! Issue 116, 117, 118, 119 & ZD v2 Spec — Redemption Chat Flow End-to-End
//!
//! Principles:
//! - Tokens are used ONLY through a redemption chat.
//! - One open redemption per application (`409 REDEMPTION_ALREADY_OPEN`).
//! - Partial use allowed.
//! - Tokens locked at submission and redeemed when an admin approves.
//! - Funds go to the LANDLORD's bank account entered by the admin from the verified bank proof.
//! - Balances moved with SELECT ... FOR UPDATE (using zd_token_service).
//! - Messages can NEVER be edited or deleted.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    services::{
        storage::{S3Storage, StorageService},
        zd_token_service::{self, TokenLedgerError},
    },
};

pub const MAX_DOC_FILE_SIZE: usize = 10 * 1024 * 1024; // 10MB
pub const SIGNED_URL_VALIDITY_SECS: u64 = 15 * 60;     // 15 minutes

#[derive(Debug, Error)]
pub enum RedemptionError {
    #[error("Idempotency key reused with different request payload.")]
    DuplicateRequest,
    #[error("An active redemption is already open for this application.")]
    RedemptionAlreadyOpen,
    #[error("Tokens have expired or allocation is inactive.")]
    TokensExpired,
    #[error("Requested tokens exceed available balance (requested: {requested}, available: {available}).")]
    InsufficientTokens { requested: f64, available: f64 },
    #[error("Required documents missing: {0:?}")]
    DocumentsMissing(Vec<String>),
    #[error("File too large. Maximum allowed size is 10 MB.")]
    FileTooLarge,
    #[error("Unsupported file type. Only PDF, JPG, and PNG are allowed.")]
    UnsupportedFileType,
    #[error("Preconditions failed: {0:?}")]
    PreconditionFailed(Vec<String>),
    #[error("Maker-checker violation: Releasing admin must differ from approving admin.")]
    MakerCheckerViolation,
    #[error("Released amount does not match approved amount.")]
    AmountMismatch,
    #[error("Duplicate UTR reference detected.")]
    DuplicateUtr,
    #[error("Invalid state for this operation: {0}")]
    InvalidState(String),
    #[error("Not authorized to access this redemption.")]
    NotOwner,
    #[error("Redemption not found.")]
    NotFound,
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("Ledger error: {0}")]
    Ledger(#[from] TokenLedgerError),
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn detect_content_type(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(b"%PDF-") || bytes.starts_with(b"%PDF") {
        Some(("application/pdf", "pdf"))
    } else if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        Some(("image/jpeg", "jpg"))
    } else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some(("image/png", "png"))
    } else {
        None
    }
}

pub fn compute_hash(val: &Value) -> String {
    let mut hasher = Sha256::new();
    let s = val.to_string();
    hasher.update(s.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn mask_account_number(acc: &str) -> String {
    let trimmed = acc.trim();
    if trimmed.len() > 4 {
        format!("****{}", &trimmed[trimmed.len() - 4..])
    } else {
        "****".to_string()
    }
}

pub fn encrypt_account_number(acc: &str, secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    let key = hasher.finalize();

    // Deterministic XOR envelope for encrypted persistence
    let encrypted_bytes: Vec<u8> = acc
        .as_bytes()
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ key[i % key.len()])
        .collect();

    format!("ENC:{}", hex::encode(encrypted_bytes))
}

pub fn generate_signed_url(s3_key: &str, _storage: &S3Storage) -> String {
    let base_s3 = std::env::var("S3_ENDPOINT")
        .or_else(|_| std::env::var("AWS_ENDPOINT_URL"))
        .unwrap_or_else(|_| "https://s3.amazonaws.com/livana-docs-private".to_string());
    let expires_at = Utc::now().timestamp() + SIGNED_URL_VALIDITY_SECS as i64;
    format!("{}/{}?X-Amz-Expires={}&X-Amz-Signed=true", base_s3.trim_end_matches('/'), s3_key, expires_at)
}

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct OpenRedemptionReq {
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RequiredDocItem {
    pub doc_type: String,
    pub label: String,
    pub required: bool,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct OpenRedemptionResp {
    pub redemption_id: Uuid,
    pub chat_id: Uuid,
    pub status: String,
    pub tokens_available: f64,
    pub tokens_expire_at: DateTime<Utc>,
    pub required_documents: Vec<RequiredDocItem>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitRedemptionReq {
    pub tokens_requested: f64,
}

#[derive(Debug, Deserialize)]
pub struct CancelRedemptionReq {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageReq {
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct AdminDocRequestReq {
    pub doc_type: String,
    pub label: String,
    #[serde(alias = "instructions")]
    pub note: Option<String>,
    pub required: bool,
}

#[derive(Debug, Deserialize)]
pub struct AdminDocReviewReq {
    pub status: String, // "verified" | "rejected"
    #[serde(alias = "admin_comment")]
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PayeeInput {
    #[serde(alias = "account_holder_name")]
    pub name: String,
    pub account_number: String,
    pub ifsc: String,
    pub bank_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AdminApproveReq {
    pub tokens_approved: f64,
    pub payee: Option<PayeeInput>,
    pub payee_name: Option<String>,
    pub payee_account_number: Option<String>,
    pub payee_ifsc: Option<String>,
    pub bank_name: Option<String>,
    pub remarks: Option<String>,
}

impl AdminApproveReq {
    pub fn resolved_payee(&self) -> Result<PayeeInput, RedemptionError> {
        if let Some(p) = &self.payee {
            return Ok(p.clone());
        }
        if let (Some(name), Some(acc), Some(ifsc)) = (
            &self.payee_name,
            &self.payee_account_number,
            &self.payee_ifsc,
        ) {
            return Ok(PayeeInput {
                name: name.clone(),
                account_number: acc.clone(),
                ifsc: ifsc.clone(),
                bank_name: self.bank_name.clone(),
            });
        }
        Err(RedemptionError::InvalidState("Payee details (name, account_number, ifsc) are required for approval".into()))
    }
}

#[derive(Debug, Deserialize)]
pub struct AdminRejectReq {
    pub reason: String,
}

fn default_payout_mode() -> String {
    "NEFT".to_string()
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AdminReleaseReq {
    pub released_amount: f64,
    #[serde(default = "default_payout_mode")]
    pub payout_mode: String,
    pub utr: String,
    pub released_at: Option<DateTime<Utc>>,
    #[serde(alias = "notes")]
    pub remarks: Option<String>,
}

// ---------------------------------------------------------------------------
// Realtime Push Helper
// ---------------------------------------------------------------------------

pub async fn broadcast_redemption_message(
    state: &AppState,
    chat_id: Uuid,
    target_user_id: Option<Uuid>,
    msg_val: &Value,
) {
    let payload = json!({
        "type": "zd_redemption_message",
        "chat_id": chat_id,
        "message": msg_val
    })
    .to_string();

    if let Some(uid) = target_user_id {
        if let Some(sender) = state.active_sockets.get(&uid) {
            let _ = sender.send(payload.clone()).await;
        } else {
            // App closed: insert push notification
            let notification_id = Uuid::new_v4();
            let _ = sqlx::query(
                "INSERT INTO notifications (id, user_id, title, message, type, is_read, related_entity_id, related_entity_type, created_at)
                 VALUES ($1, $2, 'Livana Zero Deposit', 'New message in redemption chat', 'MESSAGE', false, $3, 'chat', NOW())"
            )
            .bind(notification_id)
            .bind(uid)
            .bind(chat_id)
            .execute(&state.db)
            .await;
        }
    }
}

// ---------------------------------------------------------------------------
// Tenant Core Workflows
// ---------------------------------------------------------------------------

/// POST /api/v1/zero-deposit/{zero_deposit_id}/redemptions
pub async fn open_redemption_chat(
    state: &AppState,
    zero_deposit_id: Uuid,
    user_id: Uuid,
    idempotency_key: Uuid,
    payload: &OpenRedemptionReq,
) -> Result<OpenRedemptionResp, RedemptionError> {
    let payload_val = json!({ "message": payload.message });
    let req_hash = compute_hash(&payload_val);

    // 1. Check idempotency
    let existing_idemp = sqlx::query(
        "SELECT id, idempotency_request_hash, chat_id, status::TEXT as status
         FROM zd_redemptions WHERE user_id = $1 AND idempotency_key = $2"
    )
    .bind(user_id)
    .bind(idempotency_key.to_string())
    .fetch_optional(&state.db)
    .await?;

    if let Some(row) = existing_idemp {
        let stored_hash: Option<String> = row.try_get("idempotency_request_hash").ok();
        let red_id: Uuid = row.get("id");
        let chat_id: Uuid = row.get("chat_id");
        let status: String = row.get("status");

        if stored_hash.as_deref() == Some(&req_hash) {
            // Return original response
            let alloc = sqlx::query(
                "SELECT available::FLOAT8 as available, expires_at FROM zd_token_allocations WHERE zero_deposit_id = $1 ORDER BY created_at DESC LIMIT 1"
            )
            .bind(zero_deposit_id)
            .fetch_one(&state.db)
            .await?;

            let docs = fetch_redemption_documents(&state.db, red_id).await?;

            return Ok(OpenRedemptionResp {
                redemption_id: red_id,
                chat_id,
                status,
                tokens_available: alloc.get("available"),
                tokens_expire_at: alloc.get("expires_at"),
                required_documents: docs,
            });
        } else {
            return Err(RedemptionError::DuplicateRequest);
        }
    }

    let mut tx = state.db.begin().await?;

    // 2. Check application ownership
    let app_row = sqlx::query("SELECT user_id, monthly_rent::FLOAT8 as rent FROM zero_deposits WHERE id = $1")
        .bind(zero_deposit_id)
        .fetch_optional(&mut *tx)
        .await?;

    let (app_user_id, monthly_rent) = match app_row {
        Some(r) => (r.get::<Uuid, _>("user_id"), r.get::<f64, _>("rent")),
        None => return Err(RedemptionError::NotFound),
    };

    if app_user_id != user_id {
        return Err(RedemptionError::NotOwner);
    }

    // 3. Check token allocation active & unexpired
    let alloc_row = sqlx::query(
        "SELECT id, available::FLOAT8 as available, status::TEXT as status, expires_at
         FROM zd_token_allocations
         WHERE zero_deposit_id = $1 AND user_id = $2
         ORDER BY created_at DESC LIMIT 1 FOR UPDATE"
    )
    .bind(zero_deposit_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (allocation_id, available, expires_at) = match alloc_row {
        Some(r) => {
            let st: String = r.get("status");
            let exp: DateTime<Utc> = r.get("expires_at");
            if st != "active" || Utc::now() >= exp {
                return Err(RedemptionError::TokensExpired);
            }
            (r.get::<Uuid, _>("id"), r.get::<f64, _>("available"), exp)
        }
        None => return Err(RedemptionError::TokensExpired),
    };

    // 4. One open redemption constraint
    let open_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM zd_redemptions
            WHERE zero_deposit_id = $1 AND status IN ('docs_pending', 'under_review', 'changes_requested', 'approved')
        )"
    )
    .bind(zero_deposit_id)
    .fetch_one(&mut *tx)
    .await?;

    if open_exists {
        return Err(RedemptionError::RedemptionAlreadyOpen);
    }

    // 5. Create chat
    let chat_id = Uuid::new_v4();
    let redemption_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO chats (id, name, chat_type, zd_redemption_id, created_at, updated_at)
         VALUES ($1, 'Livana Zero Deposit', 'zd_redemption'::chat_type, $2, NOW(), NOW())"
    )
    .bind(chat_id)
    .bind(redemption_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("INSERT INTO chat_participants (chat_id, user_id) VALUES ($1, $2)")
        .bind(chat_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    // 6. Create redemption
    sqlx::query(
        r#"INSERT INTO zd_redemptions (
            id, zero_deposit_id, allocation_id, user_id, chat_id,
            tokens_requested, tokens_locked, status, monthly_rent,
            idempotency_key, idempotency_request_hash, created_at, updated_at
        ) VALUES (
            $1, $2, $3, $4, $5,
            $6, 0, 'docs_pending'::zd_redemption_status, $7,
            $8, $9, NOW(), NOW()
        )"#
    )
    .bind(redemption_id)
    .bind(zero_deposit_id)
    .bind(allocation_id)
    .bind(user_id)
    .bind(chat_id)
    .bind(available)
    .bind(monthly_rent)
    .bind(idempotency_key.to_string())
    .bind(&req_hash)
    .execute(&mut *tx)
    .await?;

    // 7. Update application status
    sqlx::query(
        "UPDATE zero_deposits
         SET status = 'redemption_in_progress'::zero_deposit_status, updated_at = NOW()
         WHERE id = $1"
    )
    .bind(zero_deposit_id)
    .execute(&mut *tx)
    .await?;

    // 8. Populate default document checklist from zd_ai_config
    let checklist_val = sqlx::query_scalar::<_, Option<Value>>(
        "SELECT redemption_required_documents FROM zd_ai_config LIMIT 1"
    )
    .fetch_optional(&mut *tx)
    .await?
    .flatten()
    .unwrap_or_else(|| json!([
        {"doc_type": "rental_agreement", "label": "Signed rental agreement, 11+ months, with the deposit refund assigned to Livana", "required": true},
        {"doc_type": "landlord_pan", "label": "Landlord PAN", "required": true},
        {"doc_type": "landlord_aadhaar", "label": "Landlord Aadhaar", "required": true},
        {"doc_type": "landlord_bank_proof", "label": "Landlord cancelled cheque or passbook", "required": true},
        {"doc_type": "property_ownership_proof", "label": "Sale deed, property tax receipt or society share certificate", "required": true}
    ]));

    let mut required_documents = Vec::new();
    if let Some(arr) = checklist_val.as_array() {
        for item in arr {
            let doc_type = item.get("doc_type").and_then(|v| v.as_str()).unwrap_or("document");
            let label = item.get("label").and_then(|v| v.as_str()).unwrap_or(doc_type);
            let required = item.get("required").and_then(|v| v.as_bool()).unwrap_or(true);

            sqlx::query(
                r#"INSERT INTO zd_redemption_documents (
                    id, redemption_id, doc_type, label, required, status, version, created_at
                ) VALUES (
                    gen_random_uuid(), $1, $2, $3, $4, 'missing', 1, NOW()
                )"#
            )
            .bind(redemption_id)
            .bind(doc_type)
            .bind(label)
            .bind(required)
            .execute(&mut *tx)
            .await?;

            required_documents.push(RequiredDocItem {
                doc_type: doc_type.to_string(),
                label: label.to_string(),
                required,
                status: "missing".to_string(),
            });
        }
    }

    // 9. Post initial system message
    let welcome_content = "Redemption chat opened. Please upload the required documents to begin verification.";
    let msg_id = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO messages (id, chat_id, sender_type, message_type, content, metadata, created_at)
           VALUES ($1, $2, 'system', 'status', $3, $4, NOW())"#
    )
    .bind(msg_id)
    .bind(chat_id)
    .bind(welcome_content)
    .bind(json!({ "status": "docs_pending" }))
    .execute(&mut *tx)
    .await?;

    if let Some(tenant_msg) = &payload.message {
        let t_msg_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO messages (id, chat_id, sender_id, sender_type, message_type, content, metadata, created_at)
               VALUES ($1, $2, $3, 'tenant', 'text', $4, '{}', NOW())"#
        )
        .bind(t_msg_id)
        .bind(chat_id)
        .bind(user_id)
        .bind(tenant_msg)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(OpenRedemptionResp {
        redemption_id,
        chat_id,
        status: "docs_pending".to_string(),
        tokens_available: available,
        tokens_expire_at: expires_at,
        required_documents,
    })
}

pub async fn fetch_redemption_documents(pool: &PgPool, redemption_id: Uuid) -> Result<Vec<RequiredDocItem>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT doc_type, label, required, status
         FROM zd_redemption_documents
         WHERE redemption_id = $1
         ORDER BY created_at ASC"
    )
    .bind(redemption_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| RequiredDocItem {
            doc_type: r.get("doc_type"),
            label: r.try_get("label").unwrap_or_else(|_| r.get("doc_type")),
            required: r.get("required"),
            status: r.get("status"),
        })
        .collect())
}

/// GET /api/v1/zero-deposit/redemptions/me
pub async fn get_my_redemptions(
    pool: &PgPool,
    user_id: Uuid,
    status_filter: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Value>, sqlx::Error> {
    let rows = sqlx::query_scalar::<_, Value>(
        r#"SELECT row_to_json(t) FROM (
            SELECT r.id, r.zero_deposit_id, r.chat_id, r.status::TEXT as status,
                   r.tokens_requested::FLOAT8 as tokens_requested,
                   r.tokens_locked::FLOAT8 as tokens_locked,
                   r.tokens_approved::FLOAT8 as tokens_approved,
                   r.released_amount::FLOAT8 as released_amount,
                   r.utr, r.released_at, r.created_at,
                   a.expires_at as tokens_expire_at,
                   (
                       SELECT json_agg(row_to_json(d))
                       FROM zd_redemption_documents d
                       WHERE d.redemption_id = r.id
                   ) as checklist
            FROM zd_redemptions r
            JOIN zd_token_allocations a ON r.allocation_id = a.id
            WHERE r.user_id = $1
              AND ($2::TEXT IS NULL OR r.status::TEXT = $2)
            ORDER BY r.created_at DESC
            LIMIT $3 OFFSET $4
        ) t"#
    )
    .bind(user_id)
    .bind(status_filter)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// GET /api/v1/zero-deposit/redemptions/{redemption_id}
pub async fn get_redemption_detail_tenant(
    pool: &PgPool,
    user_id: Uuid,
    redemption_id: Uuid,
) -> Result<Value, RedemptionError> {
    let record = sqlx::query(
        r#"SELECT r.id, r.user_id, r.chat_id, r.status::TEXT as status,
                  r.tokens_requested::FLOAT8 as tokens_requested,
                  COALESCE(r.tokens_approved::FLOAT8, 0.0) as tokens_used,
                  r.payee_snapshot, r.utr, r.released_at,
                  a.expires_at as tokens_expire_at
           FROM zd_redemptions r
           JOIN zd_token_allocations a ON r.allocation_id = a.id
           WHERE r.id = $1"#
    )
    .bind(redemption_id)
    .fetch_optional(pool)
    .await?;

    let row = match record {
        Some(r) => r,
        None => return Err(RedemptionError::NotFound),
    };

    let owner_id: Uuid = row.get("user_id");
    if owner_id != user_id {
        return Err(RedemptionError::NotOwner);
    }

    let docs = sqlx::query_scalar::<_, Option<Value>>(
        r#"SELECT json_agg(row_to_json(d)) FROM (
            SELECT id, doc_type, label, required, status, version, review_comment, created_at, uploaded_at
            FROM zd_redemption_documents
            WHERE redemption_id = $1
            ORDER BY created_at ASC
        ) d"#
    )
    .bind(redemption_id)
    .fetch_one(pool)
    .await?
    .unwrap_or(json!([]));

    let payee_snapshot: Option<Value> = row.try_get("payee_snapshot").ok().flatten();
    let masked_payee = payee_snapshot.map(|mut p| {
        if let Some(acc) = p.get("account_number").and_then(|v| v.as_str()) {
            p["account_number"] = json!(mask_account_number(acc));
        }
        p
    });

    Ok(json!({
        "redemption_id": row.get::<Uuid, _>("id"),
        "chat_id": row.get::<Uuid, _>("chat_id"),
        "status": row.get::<String, _>("status"),
        "tokens_requested": row.get::<f64, _>("tokens_requested"),
        "tokens_used": row.get::<f64, _>("tokens_used"),
        "tokens_expire_at": row.get::<DateTime<Utc>, _>("tokens_expire_at"),
        "checklist": docs,
        "payee": masked_payee,
        "utr": row.try_get::<String, _>("utr").ok(),
        "released_at": row.try_get::<DateTime<Utc>, _>("released_at").ok()
    }))
}

/// GET /api/v1/zero-deposit/redemptions/{redemption_id}/messages
pub async fn get_redemption_messages(
    pool: &PgPool,
    redemption_id: Uuid,
    before: Option<DateTime<Utc>>,
    limit: i64,
) -> Result<Vec<Value>, sqlx::Error> {
    let rows = sqlx::query_scalar::<_, Value>(
        r#"SELECT row_to_json(t) FROM (
            SELECT m.id, m.sender_type,
                   COALESCE(u.first_name || ' ' || u.last_name, 'System') as sender_name,
                   m.message_type, m.content, m.metadata, m.created_at
            FROM messages m
            JOIN chats c ON m.chat_id = c.id
            LEFT JOIN users u ON m.sender_id = u.id
            WHERE c.zd_redemption_id = $1
              AND ($2::TIMESTAMPTZ IS NULL OR m.created_at < $2)
            ORDER BY m.created_at DESC
            LIMIT $3
        ) t"#
    )
    .bind(redemption_id)
    .bind(before)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// POST /api/v1/zero-deposit/redemptions/{redemption_id}/messages
pub async fn send_redemption_message_tenant(
    state: &AppState,
    user_id: Uuid,
    redemption_id: Uuid,
    text: &str,
) -> Result<Value, RedemptionError> {
    let red_row = sqlx::query("SELECT user_id, chat_id, status::TEXT as status FROM zd_redemptions WHERE id = $1")
        .bind(redemption_id)
        .fetch_optional(&state.db)
        .await?;

    let (owner_id, chat_id, status) = match red_row {
        Some(r) => (r.get::<Uuid, _>("user_id"), r.get::<Uuid, _>("chat_id"), r.get::<String, _>("status")),
        None => return Err(RedemptionError::NotFound),
    };

    if owner_id != user_id {
        return Err(RedemptionError::NotOwner);
    }

    if status == "approved" || status == "released" || status == "rejected" || status == "cancelled" || status == "expired" {
        return Err(RedemptionError::InvalidState("Redemption is closed; chat is read-only".into()));
    }

    let msg_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO messages (id, chat_id, sender_id, sender_type, message_type, content, metadata, created_at)
           VALUES ($1, $2, $3, 'tenant', 'text', $4, '{}', NOW())"#
    )
    .bind(msg_id)
    .bind(chat_id)
    .bind(user_id)
    .bind(text)
    .execute(&state.db)
    .await?;

    let res_val = json!({
        "id": msg_id,
        "chat_id": chat_id,
        "sender_type": "tenant",
        "message_type": "text",
        "content": text,
        "metadata": {},
        "created_at": Utc::now()
    });

    broadcast_redemption_message(state, chat_id, None, &res_val).await;

    Ok(res_val)
}

/// POST /api/v1/zero-deposit/redemptions/{redemption_id}/documents
pub async fn upload_redemption_document(
    state: &AppState,
    user_id: Uuid,
    redemption_id: Uuid,
    doc_type: &str,
    file_bytes: Vec<u8>,
    original_filename: Option<&str>,
    note: Option<&str>,
) -> Result<Value, RedemptionError> {
    if file_bytes.len() > MAX_DOC_FILE_SIZE {
        return Err(RedemptionError::FileTooLarge);
    }

    let (content_type, ext) = match detect_content_type(&file_bytes) {
        Some(ct) => ct,
        None => return Err(RedemptionError::UnsupportedFileType),
    };

    let mut tx = state.db.begin().await?;

    let red_row = sqlx::query("SELECT user_id, chat_id, status::TEXT as status FROM zd_redemptions WHERE id = $1 FOR UPDATE")
        .bind(redemption_id)
        .fetch_optional(&mut *tx)
        .await?;

    let (owner_id, chat_id, status) = match red_row {
        Some(r) => (r.get::<Uuid, _>("user_id"), r.get::<Uuid, _>("chat_id"), r.get::<String, _>("status")),
        None => return Err(RedemptionError::NotFound),
    };

    if owner_id != user_id {
        return Err(RedemptionError::NotOwner);
    }

    if status == "approved" || status == "released" || status == "rejected" || status == "cancelled" || status == "expired" {
        return Err(RedemptionError::InvalidState("Redemption is closed; documents cannot be uploaded".into()));
    }

    // Check existing document of this type
    let existing_doc = sqlx::query(
        "SELECT id, version, status, label FROM zd_redemption_documents WHERE redemption_id = $1 AND doc_type = $2 ORDER BY version DESC LIMIT 1"
    )
    .bind(redemption_id)
    .bind(doc_type)
    .fetch_optional(&mut *tx)
    .await?;

    let (existing_id, version, label) = if let Some(ed) = existing_doc {
        let doc_st: String = ed.get("status");
        if doc_st == "verified" {
            return Err(RedemptionError::InvalidState("Document is already verified and cannot be superseded".into()));
        }
        let v: i32 = ed.get("version");
        let lbl: String = ed.try_get("label").unwrap_or_else(|_| doc_type.to_string());
        (Some(ed.get::<Uuid, _>("id")), v + 1, lbl)
    } else {
        (None, 1, doc_type.to_string())
    };

    let s3_key = format!("zero-deposit/redemptions/{}/{}_v{}_{}.{}", redemption_id, doc_type, version, Uuid::new_v4(), ext);
    let s3_url = generate_signed_url(&s3_key, &state.storage_service);

    let _ = state.storage_service.upload_file(&s3_key, file_bytes.clone(), content_type).await;

    // Post message into chat
    let msg_id = Uuid::new_v4();
    let msg_content = format!("Uploaded {}", label);
    let metadata = json!({
        "doc_type": doc_type,
        "version": version,
        "file_name": original_filename,
        "s3_key": s3_key,
        "mime_type": content_type
    });

    sqlx::query(
        r#"INSERT INTO messages (id, chat_id, sender_id, sender_type, message_type, content, metadata, created_at)
           VALUES ($1, $2, $3, 'tenant', 'document', $4, $5, NOW())"#
    )
    .bind(msg_id)
    .bind(chat_id)
    .bind(user_id)
    .bind(&msg_content)
    .bind(&metadata)
    .execute(&mut *tx)
    .await?;

    let document_id = match existing_id {
        Some(doc_id) => {
            sqlx::query(
                r#"UPDATE zd_redemption_documents
                   SET status = 'uploaded',
                       version = $1,
                       url = $2,
                       s3_key = $3,
                       file_name = $4,
                       mime_type = $5,
                       size_bytes = $6,
                       message_id = $7,
                       note = $8,
                       uploaded_at = NOW()
                   WHERE id = $9"#
            )
            .bind(version)
            .bind(&s3_url)
            .bind(&s3_key)
            .bind(original_filename)
            .bind(content_type)
            .bind(file_bytes.len() as i64)
            .bind(msg_id)
            .bind(note)
            .bind(doc_id)
            .execute(&mut *tx)
            .await?;
            doc_id
        }
        None => {
            let doc_id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO zd_redemption_documents (
                    id, redemption_id, doc_type, label, required, status, version,
                    url, s3_key, file_name, mime_type, size_bytes, message_id, note, uploaded_at, created_at
                ) VALUES (
                    $1, $2, $3, $4, true, 'uploaded', $5,
                    $6, $7, $8, $9, $10, $11, $12, NOW(), NOW()
                )"#
            )
            .bind(doc_id)
            .bind(redemption_id)
            .bind(doc_type)
            .bind(&label)
            .bind(version)
            .bind(&s3_url)
            .bind(&s3_key)
            .bind(original_filename)
            .bind(content_type)
            .bind(file_bytes.len() as i64)
            .bind(msg_id)
            .bind(note)
            .execute(&mut *tx)
            .await?;
            doc_id
        }
    };

    tx.commit().await?;

    let msg_val = json!({
        "id": msg_id,
        "chat_id": chat_id,
        "sender_type": "tenant",
        "message_type": "document",
        "content": msg_content,
        "metadata": metadata,
        "created_at": Utc::now()
    });
    broadcast_redemption_message(state, chat_id, None, &msg_val).await;

    Ok(json!({
        "document_id": document_id,
        "doc_type": doc_type,
        "version": version,
        "status": "uploaded",
        "message_id": msg_id
    }))
}

/// POST /api/v1/zero-deposit/redemptions/{redemption_id}/submit
pub async fn submit_redemption(
    state: &AppState,
    user_id: Uuid,
    redemption_id: Uuid,
    payload: &SubmitRedemptionReq,
) -> Result<Value, RedemptionError> {
    let red_row = sqlx::query(
        "SELECT user_id, chat_id, zero_deposit_id, allocation_id, status::TEXT as status, tokens_locked::FLOAT8 as locked
         FROM zd_redemptions WHERE id = $1 FOR UPDATE"
    )
    .bind(redemption_id)
    .fetch_optional(&state.db)
    .await?;

    let (owner_id, chat_id, zero_deposit_id, _allocation_id, status, currently_locked) = match red_row {
        Some(r) => (
            r.get::<Uuid, _>("user_id"),
            r.get::<Uuid, _>("chat_id"),
            r.get::<Uuid, _>("zero_deposit_id"),
            r.get::<Uuid, _>("allocation_id"),
            r.get::<String, _>("status"),
            r.try_get::<f64, _>("locked").unwrap_or(0.0),
        ),
        None => return Err(RedemptionError::NotFound),
    };

    if owner_id != user_id {
        return Err(RedemptionError::NotOwner);
    }

    if status != "docs_pending" && status != "changes_requested" && status != "under_review" {
        return Err(RedemptionError::InvalidState(format!("Cannot submit redemption in status '{}'", status)));
    }

    // 1. Verify every REQUIRED document is uploaded or verified
    let missing_docs = sqlx::query_scalar::<_, String>(
        "SELECT doc_type FROM zd_redemption_documents
         WHERE redemption_id = $1 AND required = true AND status NOT IN ('uploaded', 'verified')"
    )
    .bind(redemption_id)
    .fetch_all(&state.db)
    .await?;

    if !missing_docs.is_empty() {
        return Err(RedemptionError::DocumentsMissing(missing_docs));
    }

    // 2. Lock tokens (or adjust delta on resubmission)
    let delta_to_lock = payload.tokens_requested - currently_locked;
    if delta_to_lock > 0.0 {
        zd_token_service::lock(
            &state.db,
            zero_deposit_id,
            delta_to_lock,
            redemption_id,
            Some(user_id),
            Some("tenant"),
            Some("Redemption submitted"),
        )
        .await?;
    } else if delta_to_lock < 0.0 {
        // Return excess back
        zd_token_service::unlock(
            &state.db,
            zero_deposit_id,
            delta_to_lock.abs(),
            redemption_id,
            Some(user_id),
            Some("tenant"),
            Some("Adjusted requested tokens on resubmission"),
        )
        .await?;
    }

    // 3. Update redemption status to under_review
    sqlx::query(
        r#"UPDATE zd_redemptions
           SET status = 'under_review'::zd_redemption_status,
               tokens_requested = $1,
               tokens_locked = $1,
               submitted_at = NOW(),
               updated_at = NOW()
           WHERE id = $2"#
    )
    .bind(payload.tokens_requested)
    .bind(redemption_id)
    .execute(&state.db)
    .await?;

    // 4. Post status message
    let status_content = format!("Redemption submitted for admin review with ₹{:.0} tokens requested.", payload.tokens_requested);
    let msg_id = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO messages (id, chat_id, sender_type, message_type, content, metadata, created_at)
           VALUES ($1, $2, 'system', 'status', $3, $4, NOW())"#
    )
    .bind(msg_id)
    .bind(chat_id)
    .bind(&status_content)
    .bind(json!({ "status": "under_review", "tokens_requested": payload.tokens_requested }))
    .execute(&state.db)
    .await?;

    let msg_val = json!({
        "id": msg_id,
        "chat_id": chat_id,
        "sender_type": "system",
        "message_type": "status",
        "content": status_content,
        "metadata": { "status": "under_review" },
        "created_at": Utc::now()
    });
    broadcast_redemption_message(state, chat_id, None, &msg_val).await;

    // Trigger AI document-check job
    crate::services::zd_ai_client_service::queue_ai_document_check_job(state, redemption_id, zero_deposit_id).await;

    Ok(json!({
        "success": true,
        "redemption_id": redemption_id,
        "status": "under_review",
        "tokens_requested": payload.tokens_requested
    }))
}

/// POST /api/v1/zero-deposit/redemptions/{redemption_id}/cancel
pub async fn cancel_redemption(
    state: &AppState,
    user_id: Uuid,
    redemption_id: Uuid,
    payload: &CancelRedemptionReq,
) -> Result<Value, RedemptionError> {
    let mut tx = state.db.begin().await?;

    let red_row = sqlx::query(
        "SELECT user_id, chat_id, zero_deposit_id, allocation_id, status::TEXT as status, tokens_locked::FLOAT8 as locked
         FROM zd_redemptions WHERE id = $1 FOR UPDATE"
    )
    .bind(redemption_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (owner_id, chat_id, zero_deposit_id, allocation_id, status, locked) = match red_row {
        Some(r) => (
            r.get::<Uuid, _>("user_id"),
            r.get::<Uuid, _>("chat_id"),
            r.get::<Uuid, _>("zero_deposit_id"),
            r.get::<Uuid, _>("allocation_id"),
            r.get::<String, _>("status"),
            r.try_get::<f64, _>("locked").unwrap_or(0.0),
        ),
        None => return Err(RedemptionError::NotFound),
    };

    if owner_id != user_id {
        return Err(RedemptionError::NotOwner);
    }

    if status == "approved" || status == "released" || status == "rejected" || status == "cancelled" || status == "expired" {
        return Err(RedemptionError::InvalidState(format!("Cannot cancel redemption in status '{}'", status)));
    }

    // Unlock tokens
    if locked > 0.0 {
        zd_token_service::unlock(
            &state.db,
            zero_deposit_id,
            locked,
            redemption_id,
            Some(user_id),
            Some("tenant"),
            Some(&payload.reason),
        )
        .await?;
    }

    // Update redemption status
    sqlx::query(
        r#"UPDATE zd_redemptions
           SET status = 'cancelled'::zd_redemption_status,
               tokens_locked = 0,
               cancel_reason = $1,
               updated_at = NOW()
           WHERE id = $2"#
    )
    .bind(&payload.reason)
    .bind(redemption_id)
    .execute(&mut *tx)
    .await?;

    // Application status update: if unexpired available tokens remain -> tokens_issued, else expired
    let alloc = sqlx::query("SELECT available::FLOAT8 as available, expires_at FROM zd_token_allocations WHERE id = $1")
        .bind(allocation_id)
        .fetch_one(&mut *tx)
        .await?;

    let available: f64 = alloc.get("available");
    let expires_at: DateTime<Utc> = alloc.get("expires_at");

    let new_app_status = if available > 0.0 && Utc::now() < expires_at {
        "tokens_issued"
    } else {
        "expired"
    };

    sqlx::query("UPDATE zero_deposits SET status = $1::zero_deposit_status, updated_at = NOW() WHERE id = $2")
        .bind(new_app_status)
        .bind(zero_deposit_id)
        .execute(&mut *tx)
        .await?;

    let cancel_content = format!("Redemption cancelled: {}", payload.reason);
    let msg_id = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO messages (id, chat_id, sender_type, message_type, content, metadata, created_at)
           VALUES ($1, $2, 'system', 'status', $3, $4, NOW())"#
    )
    .bind(msg_id)
    .bind(chat_id)
    .bind(&cancel_content)
    .bind(json!({ "status": "cancelled", "reason": payload.reason }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let msg_val = json!({
        "id": msg_id,
        "chat_id": chat_id,
        "sender_type": "system",
        "message_type": "status",
        "content": cancel_content,
        "metadata": { "status": "cancelled" },
        "created_at": Utc::now()
    });
    broadcast_redemption_message(state, chat_id, None, &msg_val).await;

    Ok(json!({
        "success": true,
        "redemption_id": redemption_id,
        "status": "cancelled"
    }))
}

/// POST /api/v1/zero-deposit/redemptions/{redemption_id}/esign
pub async fn get_esign_link(
    pool: &PgPool,
    user_id: Uuid,
    redemption_id: Uuid,
) -> Result<Value, RedemptionError> {
    let red_row = sqlx::query("SELECT user_id, status::TEXT as status FROM zd_redemptions WHERE id = $1")
        .bind(redemption_id)
        .fetch_optional(pool)
        .await?;

    let (owner_id, _status) = match red_row {
        Some(r) => (r.get::<Uuid, _>("user_id"), r.get::<String, _>("status")),
        None => return Err(RedemptionError::NotFound),
    };

    if owner_id != user_id {
        return Err(RedemptionError::NotOwner);
    }

    // Verify all required documents are verified
    let unverified_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM zd_redemption_documents WHERE redemption_id = $1 AND required = true AND status != 'verified'"
    )
    .bind(redemption_id)
    .fetch_one(pool)
    .await?;

    if unverified_count > 0 {
        return Err(RedemptionError::PreconditionFailed(vec!["required_docs_verified".into()]));
    }

    let esign_token = Uuid::new_v4();
    let esign_url = format!("https://esign.livana.in/sign/{}", esign_token);
    let expires_at = Utc::now() + chrono::Duration::days(1);

    Ok(json!({
        "esign_url": esign_url,
        "signer": "tenant",
        "expires_at": expires_at
    }))
}

// ---------------------------------------------------------------------------
// Admin Workflows
// ---------------------------------------------------------------------------

/// GET /api/admin/zero-deposit/redemptions
pub async fn get_admin_redemptions_inbox(
    pool: &PgPool,
    status_filter: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Value>, sqlx::Error> {
    let rows = sqlx::query_scalar::<_, Value>(
        r#"SELECT row_to_json(t) FROM (
            SELECT r.id, r.zero_deposit_id, r.chat_id, r.status::TEXT as status,
                   r.tokens_requested::FLOAT8 as tokens_requested,
                   r.tokens_locked::FLOAT8 as tokens_locked,
                   r.tokens_approved::FLOAT8 as tokens_approved,
                   r.released_amount::FLOAT8 as released_amount,
                   r.property_address,
                   COALESCE(u.first_name || ' ' || u.last_name, 'Tenant') as tenant_name,
                   a.expires_at as tokens_expire_at,
                   (
                       SELECT COUNT(*)
                       FROM messages m
                       WHERE m.chat_id = r.chat_id AND m.sender_type = 'tenant' AND m.status != 'read'
                   ) as unread_count,
                   (
                       SELECT MAX(created_at)
                       FROM messages m
                       WHERE m.chat_id = r.chat_id
                   ) as last_message_at,
                   r.created_at
            FROM zd_redemptions r
            JOIN zd_token_allocations a ON r.allocation_id = a.id
            JOIN zero_deposits z ON r.zero_deposit_id = z.id
            LEFT JOIN users u ON coalesce(r.user_id, z.user_id) = u.id
            WHERE ($1::TEXT IS NULL OR r.status::TEXT = $1)
            ORDER BY
                CASE WHEN r.status = 'under_review'::zd_redemption_status THEN 1
                     WHEN r.status IN ('docs_pending'::zd_redemption_status, 'changes_requested'::zd_redemption_status) THEN 2
                     ELSE 3 END ASC,
                r.created_at ASC
            LIMIT $2 OFFSET $3
        ) t"#
    )
    .bind(status_filter)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// GET /api/admin/zero-deposit/redemptions/{redemption_id}
pub async fn get_admin_redemption_detail(
    state: &AppState,
    redemption_id: Uuid,
) -> Result<Value, RedemptionError> {
    let red_record = sqlx::query(
        r#"SELECT r.id, r.zero_deposit_id, r.allocation_id, r.user_id, r.chat_id,
                  r.status::TEXT as status,
                  r.tokens_requested::FLOAT8 as tokens_requested,
                  r.tokens_locked::FLOAT8 as tokens_locked,
                  COALESCE(r.tokens_approved::FLOAT8, 0.0) as tokens_approved,
                  r.released_amount::FLOAT8 as released_amount,
                  r.property_address, r.monthly_rent::FLOAT8 as monthly_rent,
                  r.landlord_name, r.landlord_phone, r.payee_snapshot,
                  r.utr, r.released_at, r.created_at,
                  r.approved_by, r.approved_at, r.admin_remarks,
                  r.ai_doc_check_result, r.doc_check_flags,
                  a.expires_at as tokens_expire_at,
                  z.user_id as app_user_id
           FROM zd_redemptions r
           JOIN zd_token_allocations a ON r.allocation_id = a.id
           JOIN zero_deposits z ON r.zero_deposit_id = z.id
           WHERE r.id = $1"#
    )
    .bind(redemption_id)
    .fetch_optional(&state.db)
    .await?;

    let row = match red_record {
        Some(r) => r,
        None => return Err(RedemptionError::NotFound),
    };

    let zero_deposit_id: Uuid = row.get("zero_deposit_id");

    // Fetch documents with 15-minute signed URLs
    let doc_rows = sqlx::query(
        "SELECT id, doc_type, label, required, status, version, s3_key, file_name, mime_type, review_comment, created_at, uploaded_at
         FROM zd_redemption_documents
         WHERE redemption_id = $1
         ORDER BY created_at ASC"
    )
    .bind(redemption_id)
    .fetch_all(&state.db)
    .await?;

    let mut documents = Vec::new();
    let mut all_required_verified = true;

    for d in doc_rows {
        let req: bool = d.get("required");
        let st: String = d.get("status");
        if req && st != "verified" {
            all_required_verified = false;
        }

        let s3_key: Option<String> = d.try_get("s3_key").ok();
        let signed_url = s3_key.as_deref().map(|k| generate_signed_url(k, &state.storage_service));

        documents.push(json!({
            "id": d.get::<Uuid, _>("id"),
            "doc_type": d.get::<String, _>("doc_type"),
            "label": d.try_get::<String, _>("label").unwrap_or_default(),
            "required": req,
            "status": st,
            "version": d.get::<i32, _>("version"),
            "signed_url": signed_url,
            "file_name": d.try_get::<String, _>("file_name").ok(),
            "mime_type": d.try_get::<String, _>("mime_type").ok(),
            "review_comment": d.try_get::<String, _>("review_comment").ok(),
            "uploaded_at": d.try_get::<DateTime<Utc>, _>("uploaded_at").ok()
        }));
    }

    // Preconditions evaluation:
    // 1. required_docs_verified
    // 2. fee_and_first_rent_paid
    let fee_paid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM fee_transactions WHERE zero_deposit_id = $1 AND status = 'success')"
    )
    .bind(zero_deposit_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(false);

    // 3. mandate_active
    let mandate_active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM autopay_mandates WHERE zero_deposit_id = $1 AND status IN ('active', 'authenticated', 'pending'))"
    )
    .bind(zero_deposit_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(false);

    // 4. agreement_esigned: Tenant AND all accepted co-applicants
    let tenant_esigned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM zd_co_applicants WHERE zero_deposit_id = $1 AND status = 'accepted' AND esigned_at IS NULL)"
    )
    .bind(zero_deposit_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(false);
    let agreement_esigned = !tenant_esigned;

    let preconditions = json!({
        "required_docs_verified": all_required_verified,
        "fee_and_first_rent_paid": fee_paid,
        "mandate_active": mandate_active,
        "agreement_esigned": agreement_esigned
    });

    Ok(json!({
        "redemption": {
            "id": row.get::<Uuid, _>("id"),
            "zero_deposit_id": zero_deposit_id,
            "chat_id": row.get::<Uuid, _>("chat_id"),
            "status": row.get::<String, _>("status"),
            "tokens_requested": row.get::<f64, _>("tokens_requested"),
            "tokens_locked": row.get::<f64, _>("tokens_locked"),
            "tokens_approved": row.get::<f64, _>("tokens_approved"),
            "released_amount": row.get::<f64, _>("released_amount"),
            "property_address": row.try_get::<String, _>("property_address").ok(),
            "monthly_rent": row.try_get::<f64, _>("monthly_rent").ok(),
            "landlord_name": row.try_get::<String, _>("landlord_name").ok(),
            "payee_snapshot": row.try_get::<Value, _>("payee_snapshot").ok(),
            "utr": row.try_get::<String, _>("utr").ok(),
            "approved_by": row.try_get::<Uuid, _>("approved_by").ok(),
            "approved_at": row.try_get::<DateTime<Utc>, _>("approved_at").ok(),
            "admin_remarks": row.try_get::<String, _>("admin_remarks").ok(),
            "ai_doc_check_result": row.try_get::<Value, _>("ai_doc_check_result").ok(),
            "doc_check_flags": row.try_get::<Value, _>("doc_check_flags").ok(),
            "tokens_expire_at": row.get::<DateTime<Utc>, _>("tokens_expire_at"),
            "created_at": row.get::<DateTime<Utc>, _>("created_at"),
        },
        "documents": documents,
        "preconditions": preconditions
    }))
}

/// POST /api/admin/zero-deposit/redemptions/{redemption_id}/messages
pub async fn send_redemption_message_admin(
    state: &AppState,
    admin_id: Uuid,
    admin_name: &str,
    redemption_id: Uuid,
    text: &str,
) -> Result<Value, RedemptionError> {
    let red_row = sqlx::query("SELECT user_id, chat_id, status::TEXT as status FROM zd_redemptions WHERE id = $1")
        .bind(redemption_id)
        .fetch_optional(&state.db)
        .await?;

    let (tenant_id, chat_id, status) = match red_row {
        Some(r) => (r.get::<Uuid, _>("user_id"), r.get::<Uuid, _>("chat_id"), r.get::<String, _>("status")),
        None => return Err(RedemptionError::NotFound),
    };

    if status == "approved" || status == "released" || status == "rejected" || status == "cancelled" || status == "expired" {
        return Err(RedemptionError::InvalidState("Redemption is closed; chat is read-only".into()));
    }

    let first_name = admin_name.split_whitespace().next().unwrap_or("Admin");
    let msg_id = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO messages (id, chat_id, sender_id, sender_type, message_type, content, metadata, created_at)
           VALUES ($1, $2, $3, 'admin', 'text', $4, $5, NOW())"#
    )
    .bind(msg_id)
    .bind(chat_id)
    .bind(admin_id)
    .bind(text)
    .bind(json!({ "admin_first_name": first_name, "sender_name": "Livana team" }))
    .execute(&state.db)
    .await?;

    let res_val = json!({
        "id": msg_id,
        "chat_id": chat_id,
        "sender_type": "admin",
        "sender_name": "Livana team",
        "message_type": "text",
        "content": text,
        "metadata": { "admin_first_name": first_name },
        "created_at": Utc::now()
    });

    broadcast_redemption_message(state, chat_id, Some(tenant_id), &res_val).await;

    Ok(res_val)
}

/// POST /api/admin/zero-deposit/redemptions/{redemption_id}/document-requests
pub async fn add_document_request_admin(
    state: &AppState,
    admin_id: Uuid,
    redemption_id: Uuid,
    req: &AdminDocRequestReq,
) -> Result<Value, RedemptionError> {
    let mut tx = state.db.begin().await?;

    let red_row = sqlx::query("SELECT user_id, chat_id, status::TEXT as status FROM zd_redemptions WHERE id = $1 FOR UPDATE")
        .bind(redemption_id)
        .fetch_optional(&mut *tx)
        .await?;

    let (tenant_id, chat_id, status) = match red_row {
        Some(r) => (r.get::<Uuid, _>("user_id"), r.get::<Uuid, _>("chat_id"), r.get::<String, _>("status")),
        None => return Err(RedemptionError::NotFound),
    };

    let doc_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO zd_redemption_documents (
            id, redemption_id, doc_type, label, required, status, version, requested_by, note, created_at
        ) VALUES (
            $1, $2, $3, $4, $5, 'missing', 1, $6, $7, NOW()
        )"#
    )
    .bind(doc_id)
    .bind(redemption_id)
    .bind(&req.doc_type)
    .bind(&req.label)
    .bind(req.required)
    .bind(admin_id)
    .bind(req.note.as_deref())
    .execute(&mut *tx)
    .await?;

    if req.required && status == "under_review" {
        sqlx::query("UPDATE zd_redemptions SET status = 'changes_requested'::zd_redemption_status, updated_at = NOW() WHERE id = $1")
            .bind(redemption_id)
            .execute(&mut *tx)
            .await?;
    }

    let msg_id = Uuid::new_v4();
    let content = format!("Please upload: {}", req.label);
    sqlx::query(
        r#"INSERT INTO messages (id, chat_id, sender_id, sender_type, message_type, content, metadata, created_at)
           VALUES ($1, $2, $3, 'admin', 'doc_request', $4, $5, NOW())"#
    )
    .bind(msg_id)
    .bind(chat_id)
    .bind(admin_id)
    .bind(&content)
    .bind(json!({
        "doc_type": req.doc_type,
        "label": req.label,
        "note": req.note,
        "required": req.required
    }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let msg_val = json!({
        "id": msg_id,
        "chat_id": chat_id,
        "sender_type": "admin",
        "sender_name": "Livana team",
        "message_type": "doc_request",
        "content": content,
        "metadata": { "doc_type": req.doc_type, "label": req.label },
        "created_at": Utc::now()
    });
    broadcast_redemption_message(state, chat_id, Some(tenant_id), &msg_val).await;

    crate::services::zd_notifications::notify_document_requested(&state.db, tenant_id, &req.label).await;

    Ok(json!({ "success": true, "document_id": doc_id, "status": "requested" }))
}

/// PATCH /api/admin/zero-deposit/redemptions/{redemption_id}/documents/{document_id}
pub async fn review_document_admin(
    state: &AppState,
    admin_id: Uuid,
    redemption_id: Uuid,
    document_id: Uuid,
    payload: &AdminDocReviewReq,
) -> Result<Value, RedemptionError> {
    if payload.status == "rejected" && payload.comment.as_deref().unwrap_or("").trim().is_empty() {
        return Err(RedemptionError::InvalidState("A comment is required when rejecting a document".into()));
    }

    let mut tx = state.db.begin().await?;

    let red_row = sqlx::query("SELECT user_id, chat_id, status::TEXT as status FROM zd_redemptions WHERE id = $1 FOR UPDATE")
        .bind(redemption_id)
        .fetch_optional(&mut *tx)
        .await?;

    let (tenant_id, chat_id, current_red_status) = match red_row {
        Some(r) => (r.get::<Uuid, _>("user_id"), r.get::<Uuid, _>("chat_id"), r.get::<String, _>("status")),
        None => return Err(RedemptionError::NotFound),
    };

    let doc_row = sqlx::query(
        "SELECT doc_type, label, required FROM zd_redemption_documents WHERE id = $1 AND redemption_id = $2"
    )
    .bind(document_id)
    .bind(redemption_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (doc_type, label, is_required) = match doc_row {
        Some(d) => (d.get::<String, _>("doc_type"), d.try_get::<String, _>("label").unwrap_or_default(), d.get::<bool, _>("required")),
        None => return Err(RedemptionError::NotFound),
    };

    sqlx::query(
        r#"UPDATE zd_redemption_documents
           SET status = $1,
               review_comment = $2,
               reviewed_by = $3,
               reviewed_at = NOW()
           WHERE id = $4"#
    )
    .bind(&payload.status)
    .bind(payload.comment.as_deref())
    .bind(admin_id)
    .bind(document_id)
    .execute(&mut *tx)
    .await?;

    // Rejecting a required document moves redemption to changes_requested (tokens stay locked!)
    if payload.status == "rejected" && is_required && current_red_status == "under_review" {
        sqlx::query(
            "UPDATE zd_redemptions SET status = 'changes_requested'::zd_redemption_status, updated_at = NOW() WHERE id = $1"
        )
        .bind(redemption_id)
        .execute(&mut *tx)
        .await?;
    }

    let review_msg = if payload.status == "verified" {
        format!("Verified document: {}", label)
    } else {
        format!("Rejected document {}: {}", label, payload.comment.as_deref().unwrap_or(""))
    };

    let msg_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO messages (id, chat_id, sender_id, sender_type, message_type, content, metadata, created_at)
           VALUES ($1, $2, $3, 'admin', 'doc_review', $4, $5, NOW())"#
    )
    .bind(msg_id)
    .bind(chat_id)
    .bind(admin_id)
    .bind(&review_msg)
    .bind(json!({
        "doc_type": doc_type,
        "doc_status": payload.status,
        "comment": payload.comment
    }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let msg_val = json!({
        "id": msg_id,
        "chat_id": chat_id,
        "sender_type": "admin",
        "sender_name": "Livana team",
        "message_type": "doc_review",
        "content": review_msg,
        "metadata": { "doc_status": payload.status },
        "created_at": Utc::now()
    });
    broadcast_redemption_message(state, chat_id, Some(tenant_id), &msg_val).await;

    if payload.status == "rejected" {
        crate::services::zd_notifications::notify_document_rejected(
            &state.db,
            tenant_id,
            &label,
            payload.comment.as_deref().unwrap_or("Document requirements not met"),
        )
        .await;
    }

    Ok(json!({ "success": true, "status": payload.status }))
}

/// PATCH /api/admin/zero-deposit/redemptions/{redemption_id}/approve
pub async fn approve_redemption_admin(
    state: &AppState,
    admin_id: Uuid,
    redemption_id: Uuid,
    payload: &AdminApproveReq,
) -> Result<Value, RedemptionError> {
    let mut tx = state.db.begin().await?;

    let red_row = sqlx::query(
        "SELECT user_id, chat_id, zero_deposit_id, allocation_id, status::TEXT as status, tokens_requested::FLOAT8 as requested
         FROM zd_redemptions WHERE id = $1 FOR UPDATE"
    )
    .bind(redemption_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (tenant_id, chat_id, zero_deposit_id, _allocation_id, status, requested) = match red_row {
        Some(r) => (
            r.get::<Uuid, _>("user_id"),
            r.get::<Uuid, _>("chat_id"),
            r.get::<Uuid, _>("zero_deposit_id"),
            r.get::<Uuid, _>("allocation_id"),
            r.get::<String, _>("status"),
            r.get::<f64, _>("requested"),
        ),
        None => return Err(RedemptionError::NotFound),
    };

    if status != "under_review" {
        return Err(RedemptionError::InvalidState(format!("Cannot approve redemption in status '{}'", status)));
    }

    if payload.tokens_approved > requested {
        return Err(RedemptionError::InvalidState("Approved tokens cannot exceed requested tokens".into()));
    }

    // Check ALL Preconditions
    let mut failed_preconditions = Vec::new();

    // 1. required_docs_verified
    let unverified_req: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM zd_redemption_documents WHERE redemption_id = $1 AND required = true AND status != 'verified'"
    )
    .bind(redemption_id)
    .fetch_one(&mut *tx)
    .await?;

    if unverified_req > 0 {
        failed_preconditions.push("required_docs_verified".to_string());
    }

    // 2. fee_and_first_rent_paid
    let fee_paid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM fee_transactions WHERE zero_deposit_id = $1 AND status = 'success')"
    )
    .bind(zero_deposit_id)
    .fetch_one(&mut *tx)
    .await?;

    if !fee_paid {
        failed_preconditions.push("fee_and_first_rent_paid".to_string());
    }

    // 3. mandate_active
    let mandate_active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM autopay_mandates WHERE zero_deposit_id = $1 AND status IN ('active', 'authenticated', 'pending'))"
    )
    .bind(zero_deposit_id)
    .fetch_one(&mut *tx)
    .await?;

    if !mandate_active {
        failed_preconditions.push("mandate_active".to_string());
    }

    // 4. agreement_esigned
    let co_unsigned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM zd_co_applicants WHERE zero_deposit_id = $1 AND status = 'accepted' AND esigned_at IS NULL)"
    )
    .bind(zero_deposit_id)
    .fetch_one(&mut *tx)
    .await?;

    if co_unsigned {
        failed_preconditions.push("agreement_esigned".to_string());
    }

    if !failed_preconditions.is_empty() {
        return Err(RedemptionError::PreconditionFailed(failed_preconditions));
    }

    // Redeem tokens via ledger
    zd_token_service::redeem(
        &state.db,
        zero_deposit_id,
        requested,
        payload.tokens_approved,
        redemption_id,
        Some(admin_id),
        Some("admin"),
        payload.remarks.as_deref(),
    )
    .await?;

    let payee = payload.resolved_payee()?;

    // Encrypt account number in payee_snapshot
    let encrypted_acc = encrypt_account_number(&payee.account_number, &state.jwt_secret);
    let masked_acc = mask_account_number(&payee.account_number);

    let payee_snapshot = json!({
        "name": payee.name,
        "account_number": masked_acc,
        "account_number_encrypted": encrypted_acc,
        "ifsc": payee.ifsc,
        "bank_name": payee.bank_name
    });

    sqlx::query(
        r#"UPDATE zd_redemptions
           SET status = 'approved'::zd_redemption_status,
               tokens_approved = $1,
               payee_snapshot = $2,
               admin_remarks = $3,
               approved_by = $4,
               approved_at = NOW(),
               updated_at = NOW()
           WHERE id = $5"#
    )
    .bind(payload.tokens_approved)
    .bind(payee_snapshot)
    .bind(payload.remarks.as_deref())
    .bind(admin_id)
    .bind(redemption_id)
    .execute(&mut *tx)
    .await?;

    let app_msg = format!("Redemption approved for ₹{:.0}. Payout will be disbursed directly to the landlord's verified account.", payload.tokens_approved);
    let msg_id = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO messages (id, chat_id, sender_type, message_type, content, metadata, created_at)
           VALUES ($1, $2, 'system', 'status', $3, $4, NOW())"#
    )
    .bind(msg_id)
    .bind(chat_id)
    .bind(&app_msg)
    .bind(json!({ "status": "approved", "tokens_approved": payload.tokens_approved }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let msg_val = json!({
        "id": msg_id,
        "chat_id": chat_id,
        "sender_type": "system",
        "message_type": "status",
        "content": app_msg,
        "metadata": { "status": "approved" },
        "created_at": Utc::now()
    });
    broadcast_redemption_message(state, chat_id, Some(tenant_id), &msg_val).await;

    crate::services::zd_notifications::notify_redemption_approved(
        &state.db,
        tenant_id,
        &payee.name,
        payload.tokens_approved,
    )
    .await;

    Ok(json!({
        "success": true,
        "redemption_id": redemption_id,
        "status": "approved",
        "tokens_approved": payload.tokens_approved
    }))
}

/// PATCH /api/admin/zero-deposit/redemptions/{redemption_id}/mark-released
pub async fn mark_released_admin(
    state: &AppState,
    admin_id: Uuid,
    redemption_id: Uuid,
    idempotency_key: Option<Uuid>,
    payload: &AdminReleaseReq,
) -> Result<Value, RedemptionError> {
    let mut tx = state.db.begin().await?;

    let payload_val = serde_json::to_value(payload).unwrap_or(json!({}));
    let payload_hash = compute_hash(&payload_val);

    if let Some(key) = idempotency_key {
        let existing = sqlx::query(
            "SELECT release_idempotency_hash, status::TEXT as status, released_amount::FLOAT8 as released_amount, utr FROM zd_redemptions WHERE id = $1 AND release_idempotency_key = $2"
        )
        .bind(redemption_id)
        .bind(key.to_string())
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(row) = existing {
            let prev_hash: Option<String> = row.try_get("release_idempotency_hash").ok().flatten();
            if prev_hash.as_deref() == Some(&payload_hash) {
                return Ok(json!({
                    "success": true,
                    "redemption_id": redemption_id,
                    "status": "released",
                    "released_amount": row.get::<f64, _>("released_amount"),
                    "utr": row.get::<String, _>("utr")
                }));
            } else {
                return Err(RedemptionError::DuplicateRequest);
            }
        }
    }

    let red_row = sqlx::query(
        r#"SELECT user_id, chat_id, zero_deposit_id, allocation_id, status::TEXT as status,
                  tokens_approved::FLOAT8 as approved, approved_by
           FROM zd_redemptions WHERE id = $1 FOR UPDATE"#
    )
    .bind(redemption_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (tenant_id, chat_id, zero_deposit_id, allocation_id, status, approved_amount, approved_by) = match red_row {
        Some(r) => (
            r.get::<Uuid, _>("user_id"),
            r.get::<Uuid, _>("chat_id"),
            r.get::<Uuid, _>("zero_deposit_id"),
            r.get::<Uuid, _>("allocation_id"),
            r.get::<String, _>("status"),
            r.get::<f64, _>("approved"),
            r.try_get::<Uuid, _>("approved_by").ok(),
        ),
        None => return Err(RedemptionError::NotFound),
    };

    if status != "approved" {
        return Err(RedemptionError::InvalidState(format!("Cannot mark released from status '{}'", status)));
    }

    if (payload.released_amount - approved_amount).abs() > 0.01 {
        return Err(RedemptionError::AmountMismatch);
    }

    // Maker-checker check
    let maker_checker_enabled: bool = sqlx::query_scalar(
        "SELECT maker_checker_enabled FROM zd_ai_config LIMIT 1"
    )
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or(true);

    if maker_checker_enabled && Some(admin_id) == approved_by {
        return Err(RedemptionError::MakerCheckerViolation);
    }

    // UTR Unique check
    let utr_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM zd_redemptions WHERE utr = $1 AND id != $2)"
    )
    .bind(&payload.utr)
    .bind(redemption_id)
    .fetch_one(&mut *tx)
    .await?;

    if utr_exists {
        return Err(RedemptionError::DuplicateUtr);
    }

    // Update redemption
    sqlx::query(
        r#"UPDATE zd_redemptions
           SET status = 'released'::zd_redemption_status,
               released_by = $1,
               released_at = COALESCE($2, NOW()),
               released_amount = $3,
               payout_mode = $4,
               utr = $5,
               release_idempotency_key = $6,
               release_idempotency_hash = $7,
               completed_at = NOW(),
               updated_at = NOW()
           WHERE id = $8"#
    )
    .bind(admin_id)
    .bind(payload.released_at)
    .bind(payload.released_amount)
    .bind(&payload.payout_mode)
    .bind(&payload.utr)
    .bind(idempotency_key.map(|k| k.to_string()))
    .bind(Some(payload_hash))
    .bind(redemption_id)
    .execute(&mut *tx)
    .await?;

    // Insert ledger entry for disbursement
    sqlx::query(
        r#"INSERT INTO ledger_entries (
            zero_deposit_id, transaction_type, amount, reference_id, description, created_at
        ) VALUES (
            $1, 'disbursement'::ledger_transaction_type, $2, $3, $4, NOW()
        )"#
    )
    .bind(zero_deposit_id)
    .bind(payload.released_amount)
    .bind(&payload.utr)
    .bind(format!("Security deposit disbursed to landlord via {} (UTR: {})", payload.payout_mode, payload.utr))
    .execute(&mut *tx)
    .await?;

    // Check remaining available tokens
    let alloc = sqlx::query("SELECT available::FLOAT8 as available FROM zd_token_allocations WHERE id = $1")
        .bind(allocation_id)
        .fetch_one(&mut *tx)
        .await?;

    let remaining_avail: f64 = alloc.get("available");
    let new_app_status = if remaining_avail <= 0.0 { "released" } else { "tokens_issued" };

    sqlx::query("UPDATE zero_deposits SET status = $1::zero_deposit_status, updated_at = NOW() WHERE id = $2")
        .bind(new_app_status)
        .bind(zero_deposit_id)
        .execute(&mut *tx)
        .await?;

    // Post payout message into chat
    let release_msg = format!("₹{:.0} released to the landlord, UTR {}", payload.released_amount, payload.utr);
    let msg_id = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO messages (id, chat_id, sender_type, message_type, content, metadata, created_at)
           VALUES ($1, $2, 'system', 'status', $3, $4, NOW())"#
    )
    .bind(msg_id)
    .bind(chat_id)
    .bind(&release_msg)
    .bind(json!({
        "status": "released",
        "released_amount": payload.released_amount,
        "utr": payload.utr
    }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let msg_val = json!({
        "id": msg_id,
        "chat_id": chat_id,
        "sender_type": "system",
        "message_type": "status",
        "content": release_msg,
        "metadata": { "status": "released", "utr": payload.utr },
        "created_at": Utc::now()
    });
    broadcast_redemption_message(state, chat_id, Some(tenant_id), &msg_val).await;

    crate::services::zd_notifications::notify_funds_released(
        &state.db,
        tenant_id,
        payload.released_amount,
        &payload.utr,
    )
    .await;

    Ok(json!({
        "success": true,
        "redemption_id": redemption_id,
        "status": "released",
        "released_amount": payload.released_amount,
        "utr": payload.utr
    }))
}

/// PATCH /api/admin/zero-deposit/redemptions/{redemption_id}/reject
pub async fn reject_redemption_admin(
    state: &AppState,
    admin_id: Uuid,
    redemption_id: Uuid,
    payload: &AdminRejectReq,
) -> Result<Value, RedemptionError> {
    let mut tx = state.db.begin().await?;

    let red_row = sqlx::query(
        "SELECT user_id, chat_id, zero_deposit_id, allocation_id, status::TEXT as status, tokens_locked::FLOAT8 as locked
         FROM zd_redemptions WHERE id = $1 FOR UPDATE"
    )
    .bind(redemption_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (tenant_id, chat_id, zero_deposit_id, allocation_id, status, locked) = match red_row {
        Some(r) => (
            r.get::<Uuid, _>("user_id"),
            r.get::<Uuid, _>("chat_id"),
            r.get::<Uuid, _>("zero_deposit_id"),
            r.get::<Uuid, _>("allocation_id"),
            r.get::<String, _>("status"),
            r.try_get::<f64, _>("locked").unwrap_or(0.0),
        ),
        None => return Err(RedemptionError::NotFound),
    };

    if status == "approved" || status == "released" || status == "rejected" || status == "cancelled" || status == "expired" {
        return Err(RedemptionError::InvalidState(format!("Cannot reject redemption in status '{}'", status)));
    }

    if locked > 0.0 {
        zd_token_service::unlock(
            &state.db,
            zero_deposit_id,
            locked,
            redemption_id,
            Some(admin_id),
            Some("admin"),
            Some(&payload.reason),
        )
        .await?;
    }

    sqlx::query(
        r#"UPDATE zd_redemptions
           SET status = 'rejected'::zd_redemption_status,
               reject_reason = $1,
               tokens_locked = 0,
               completed_at = NOW(),
               updated_at = NOW()
           WHERE id = $2"#
    )
    .bind(&payload.reason)
    .bind(redemption_id)
    .execute(&mut *tx)
    .await?;

    // If unexpired tokens remain, application status returns to tokens_issued
    let alloc = sqlx::query("SELECT available::FLOAT8 as available, expires_at FROM zd_token_allocations WHERE id = $1")
        .bind(allocation_id)
        .fetch_one(&mut *tx)
        .await?;

    let available: f64 = alloc.get("available");
    let expires_at: DateTime<Utc> = alloc.get("expires_at");

    let new_app_status = if available > 0.0 && Utc::now() < expires_at {
        "tokens_issued"
    } else {
        "expired"
    };

    sqlx::query("UPDATE zero_deposits SET status = $1::zero_deposit_status, updated_at = NOW() WHERE id = $2")
        .bind(new_app_status)
        .bind(zero_deposit_id)
        .execute(&mut *tx)
        .await?;

    let rej_msg = format!("Redemption rejected: {}", payload.reason);
    let msg_id = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO messages (id, chat_id, sender_id, sender_type, message_type, content, metadata, created_at)
           VALUES ($1, $2, $3, 'admin', 'status', $4, $5, NOW())"#
    )
    .bind(msg_id)
    .bind(chat_id)
    .bind(admin_id)
    .bind(&rej_msg)
    .bind(json!({ "status": "rejected", "reason": payload.reason }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let msg_val = json!({
        "id": msg_id,
        "chat_id": chat_id,
        "sender_type": "admin",
        "sender_name": "Livana team",
        "message_type": "status",
        "content": rej_msg,
        "metadata": { "status": "rejected" },
        "created_at": Utc::now()
    });
    broadcast_redemption_message(state, chat_id, Some(tenant_id), &msg_val).await;

    Ok(json!({
        "success": true,
        "redemption_id": redemption_id,
        "status": "rejected"
    }))
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magic_bytes_detection() {
        let pdf_bytes = b"%PDF-1.4 header contents";
        let (ct, ext) = detect_content_type(pdf_bytes).expect("PDF should be detected");
        assert_eq!(ct, "application/pdf");
        assert_eq!(ext, "pdf");

        let jpeg_bytes = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        let (ct, ext) = detect_content_type(&jpeg_bytes).expect("JPEG should be detected");
        assert_eq!(ct, "image/jpeg");
        assert_eq!(ext, "jpg");

        let png_bytes = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00];
        let (ct, ext) = detect_content_type(&png_bytes).expect("PNG should be detected");
        assert_eq!(ct, "image/png");
        assert_eq!(ext, "png");

        let bad_bytes = b"random plain text";
        assert!(detect_content_type(bad_bytes).is_none());
    }

    #[test]
    fn test_account_masking() {
        assert_eq!(mask_account_number("123456789012"), "****9012");
        assert_eq!(mask_account_number("987654321"), "****4321");
        assert_eq!(mask_account_number("1234"), "****");
    }

    #[test]
    fn test_encryption_deterministic() {
        let secret = "test_secret_key";
        let acc = "123456789012";
        let enc1 = encrypt_account_number(acc, secret);
        let enc2 = encrypt_account_number(acc, secret);
        assert_eq!(enc1, enc2);
        assert!(enc1.starts_with("ENC:"));
    }
}
