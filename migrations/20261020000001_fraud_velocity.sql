-- Issue 96 — Cross-application anomaly detection
-- Issue 97 — Velocity checks (device / IP throttling)
--
-- Design decision (Issue 96):
--   Fraud-signal detection runs asynchronously at application time via a
--   tokio::spawn task (same pattern as risk-scoring in Issues 93/94).
--   It does NOT block POST /api/v1/loans/apply — results are stored as a
--   JSONB `fraud_signals` column on `zero_deposits` and surfaced through
--   GET /api/admin/loans/fraud-signals for admin review.
--
--   Rationale: cross-table scans over all existing applications for matching
--   device/bank fingerprints can be slow; blocking the apply response would
--   hurt UX for legitimate applicants. Admins review flagged cases and can
--   manually block/escalate.
--
-- Design decision (Issue 97):
--   Velocity tracking uses a dedicated `loan_apply_events` table as a
--   sliding-window counter (Redis-compatible but also works without Redis).
--   When Redis IS available the middleware uses atomic Redis INCR (fast path).
--   When Redis is absent it falls back to a COUNT(*) query on this table
--   (slower but correct — no silent failures).

-- ──────────────────────────────────────────────────────────────────────────
-- 1. Capture device / network metadata on the loan application (Issue 96+97)
-- ──────────────────────────────────────────────────────────────────────────

-- Normalised SHA-256 hash of "IFSC:AccountNumber" (lowercase, stripped).
-- Stored instead of the raw account number to avoid PII in the main table.
ALTER TABLE zero_deposits
    ADD COLUMN IF NOT EXISTS device_fingerprint    TEXT,
    ADD COLUMN IF NOT EXISTS bank_account_hash     TEXT,        -- SHA-256(IFSC:AccountNumber)
    ADD COLUMN IF NOT EXISTS applicant_ip          INET,
    ADD COLUMN IF NOT EXISTS stated_employer       TEXT,
    ADD COLUMN IF NOT EXISTS fraud_signals         JSONB NOT NULL DEFAULT '[]',
    ADD COLUMN IF NOT EXISTS fraud_flagged_at      TIMESTAMPTZ;

-- Store normalized bank-account-hash on loan ledger entries (Issue 96)
ALTER TABLE ledger_entries
    ADD COLUMN IF NOT EXISTS bank_account_hash     TEXT;

COMMENT ON COLUMN zero_deposits.device_fingerprint IS
    'Opaque device fingerprint string supplied by the client at apply time (Issue 96/97).';
COMMENT ON COLUMN zero_deposits.bank_account_hash IS
    'SHA-256 hex of "IFSC:account_number" (lowercased, whitespace-stripped). Used for cross-application bank-account deduplication without storing raw PII (Issue 96).';
COMMENT ON COLUMN zero_deposits.stated_employer IS
    'Stated employer or company name provided at loan application time (Issue 96).';
COMMENT ON COLUMN zero_deposits.applicant_ip IS
    'Client IP address captured at apply time for velocity throttling (Issue 97).';
COMMENT ON COLUMN zero_deposits.fraud_signals IS
    'Array of {code, label, detail, severity} objects populated by the async fraud-detection job (Issue 96).';

-- Indexes to make the anomaly-detection queries fast
CREATE INDEX IF NOT EXISTS idx_zd_device_fingerprint
    ON zero_deposits(device_fingerprint)
    WHERE device_fingerprint IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_zd_bank_account_hash
    ON zero_deposits(bank_account_hash)
    WHERE bank_account_hash IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_ledger_entries_bank_account_hash
    ON ledger_entries(bank_account_hash)
    WHERE bank_account_hash IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_zd_applicant_ip
    ON zero_deposits(applicant_ip)
    WHERE applicant_ip IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_zd_fraud_flagged
    ON zero_deposits(fraud_flagged_at)
    WHERE fraud_flagged_at IS NOT NULL;


-- ──────────────────────────────────────────────────────────────────────────
-- 2. Velocity-tracking event log (Issue 97)
--    Every call to POST /api/v1/loans/apply writes one row here.
--    The middleware counts rows in a configurable sliding window.
--    Old rows are cheap to purge (simple DELETE WHERE created_at < NOW() - interval).
-- ──────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS loan_apply_events (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    device_id       TEXT,                       -- client-supplied device fingerprint
    ip_address      INET,                       -- network peer IP
    user_id         UUID REFERENCES users(id) ON DELETE SET NULL,
    zero_deposit_id UUID REFERENCES zero_deposits(id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE loan_apply_events IS
    'Append-only log of every POST /api/v1/loans/apply attempt. Used for sliding-window velocity checks (Issue 97). Rows older than the longest window (default 1 h) can be safely purged.';

CREATE INDEX IF NOT EXISTS idx_lae_device_id
    ON loan_apply_events(device_id, created_at)
    WHERE device_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_lae_ip_address
    ON loan_apply_events(ip_address, created_at)
    WHERE ip_address IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_lae_user_id
    ON loan_apply_events(user_id, created_at)
    WHERE user_id IS NOT NULL;
