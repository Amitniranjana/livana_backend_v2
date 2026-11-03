-- =============================================================================
-- Migration: Zero Deposit V2 Foundation
-- Issue:     #107
-- Created:   2026-11-03
-- =============================================================================

-- -----------------------------------------------------------------------------
-- 1. New enum values for zero_deposit_status
-- -----------------------------------------------------------------------------
ALTER TYPE zero_deposit_status ADD VALUE IF NOT EXISTS 'ai_review';
ALTER TYPE zero_deposit_status ADD VALUE IF NOT EXISTS 'action_required';
ALTER TYPE zero_deposit_status ADD VALUE IF NOT EXISTS 'ai_manual_review';
ALTER TYPE zero_deposit_status ADD VALUE IF NOT EXISTS 'tokens_issued';
ALTER TYPE zero_deposit_status ADD VALUE IF NOT EXISTS 'redemption_in_progress';
ALTER TYPE zero_deposit_status ADD VALUE IF NOT EXISTS 'released';
ALTER TYPE zero_deposit_status ADD VALUE IF NOT EXISTS 'expired';

-- -----------------------------------------------------------------------------
-- 2. New columns on zero_deposits
-- Note: requested_deposit_amount is kept nullable for existing rows;
--       new rows should populate deposit_amount instead.
-- -----------------------------------------------------------------------------
ALTER TABLE zero_deposits
    ADD COLUMN IF NOT EXISTS tenancy_months             INTEGER,
    ADD COLUMN IF NOT EXISTS employment_type            VARCHAR(50),        -- salaried | self_employed | business_owner
    ADD COLUMN IF NOT EXISTS employer_name              VARCHAR(255),
    ADD COLUMN IF NOT EXISTS current_job_start_date     DATE,
    ADD COLUMN IF NOT EXISTS total_experience_months    INTEGER,
    ADD COLUMN IF NOT EXISTS business_start_date        DATE,
    ADD COLUMN IF NOT EXISTS variable_pay_monthly       NUMERIC(12,2),
    ADD COLUMN IF NOT EXISTS is_repeat_renter           BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS occupants                  JSONB NOT NULL DEFAULT '[]',
    ADD COLUMN IF NOT EXISTS document_urls              JSONB NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS deposit_amount             NUMERIC(12,2),
    ADD COLUMN IF NOT EXISTS fee_amount                 NUMERIC(12,2),
    ADD COLUMN IF NOT EXISTS pending_actions            TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS bureau_summary             JSONB NOT NULL DEFAULT '{}';

-- -----------------------------------------------------------------------------
-- 3. ledger_entries / ledger_transaction_type
--    'disbursement' already exists in ledger_transaction_type enum
--    (added in 20260911233000_create_zero_deposit_financials.sql) — no-op.
-- -----------------------------------------------------------------------------

-- -----------------------------------------------------------------------------
-- 4. chats table — add chat_type and zd_redemption_id
-- -----------------------------------------------------------------------------
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'chat_type') THEN
        CREATE TYPE chat_type AS ENUM ('direct', 'zd_redemption');
    END IF;
END$$;

ALTER TABLE chats
    ADD COLUMN IF NOT EXISTS chat_type          chat_type NOT NULL DEFAULT 'direct',
    ADD COLUMN IF NOT EXISTS zd_redemption_id   UUID;

-- -----------------------------------------------------------------------------
-- 5. messages table
--    message_type is a plain TEXT column (not a PG enum), so no ALTER TYPE
--    needed — new type values ('doc_request', 'doc_review', 'status') are
--    valid as soon as application code uses them.
--
--    Add sender_type, metadata; make sender_id nullable for system messages.
-- -----------------------------------------------------------------------------
ALTER TABLE messages
    ADD COLUMN IF NOT EXISTS sender_type    VARCHAR(50),    -- 'user' | 'admin' | 'system'
    ADD COLUMN IF NOT EXISTS metadata       JSONB NOT NULL DEFAULT '{}';

-- Allow sender_id to be NULL so system-generated messages don't need a user
ALTER TABLE messages ALTER COLUMN sender_id DROP NOT NULL;

-- -----------------------------------------------------------------------------
-- 6. New tables
-- -----------------------------------------------------------------------------

-- 6a. zero_deposit_ai_assessments
CREATE TABLE IF NOT EXISTS zero_deposit_ai_assessments (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    zero_deposit_id     UUID NOT NULL REFERENCES zero_deposits(id) ON DELETE CASCADE,
    assessment_id       VARCHAR(255) NOT NULL UNIQUE,   -- from AI service
    status              VARCHAR(50) NOT NULL DEFAULT 'pending',  -- pending | completed | failed
    recommendation      VARCHAR(50),                    -- auto_approve | action_required | manual_review | auto_reject
    rules_version       VARCHAR(50),
    extracted           JSONB NOT NULL DEFAULT '{}',
    ai_flags            JSONB NOT NULL DEFAULT '[]',
    admin_summary       TEXT,
    error_code          VARCHAR(100),
    error_message       TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_zd_ai_assessments_zero_deposit_id ON zero_deposit_ai_assessments(zero_deposit_id);
CREATE INDEX IF NOT EXISTS idx_zd_ai_assessments_assessment_id   ON zero_deposit_ai_assessments(assessment_id);

-- 6b. zd_tenant_tiers
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'zd_tenant_tier') THEN
        CREATE TYPE zd_tenant_tier AS ENUM ('A', 'B', 'C');
    END IF;
END$$;

CREATE TABLE IF NOT EXISTS zd_tenant_tiers (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    zero_deposit_id     UUID NOT NULL UNIQUE REFERENCES zero_deposits(id) ON DELETE CASCADE,
    tier                zd_tenant_tier NOT NULL,
    cibil_score         INTEGER,
    income_multiple     NUMERIC(5,2),
    rent_cap_pct        NUMERIC(5,2),
    assigned_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    rules_version       VARCHAR(50),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 6c. zd_token_allocations
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'zd_token_allocation_status') THEN
        CREATE TYPE zd_token_allocation_status AS ENUM ('active', 'expired', 'revoked', 'exhausted');
    END IF;
END$$;

CREATE TABLE IF NOT EXISTS zd_token_allocations (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    zero_deposit_id     UUID NOT NULL REFERENCES zero_deposits(id) ON DELETE CASCADE,
    issued              NUMERIC(12,2) NOT NULL,
    available           NUMERIC(12,2) NOT NULL,
    locked              NUMERIC(12,2) NOT NULL DEFAULT 0,
    redeemed            NUMERIC(12,2) NOT NULL DEFAULT 0,
    status              zd_token_allocation_status NOT NULL DEFAULT 'active',
    issued_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at          TIMESTAMPTZ NOT NULL,   -- always issued_at + 30 days
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_token_balance            CHECK (available + locked + redeemed <= issued),
    CONSTRAINT chk_non_negative_available   CHECK (available >= 0),
    CONSTRAINT chk_non_negative_locked      CHECK (locked >= 0),
    CONSTRAINT chk_non_negative_redeemed    CHECK (redeemed >= 0)
);

-- Only one active allocation per application
CREATE UNIQUE INDEX IF NOT EXISTS idx_zd_token_alloc_one_active
    ON zd_token_allocations(zero_deposit_id)
    WHERE status = 'active';

CREATE INDEX IF NOT EXISTS idx_zd_token_alloc_zero_deposit_id ON zd_token_allocations(zero_deposit_id);

-- 6d. zd_token_transactions (append-only ledger)
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'zd_token_operation') THEN
        CREATE TYPE zd_token_operation AS ENUM ('issue', 'lock', 'unlock', 'redeem', 'expire', 'revoke');
    END IF;
END$$;

CREATE TABLE IF NOT EXISTS zd_token_transactions (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    allocation_id       UUID NOT NULL REFERENCES zd_token_allocations(id) ON DELETE CASCADE,
    zero_deposit_id     UUID NOT NULL REFERENCES zero_deposits(id) ON DELETE CASCADE,
    operation           zd_token_operation NOT NULL,
    amount              NUMERIC(12,2) NOT NULL,
    reference_id        UUID,   -- redemption_id or other reference
    notes               TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_zd_token_txns_allocation_id   ON zd_token_transactions(allocation_id);
CREATE INDEX IF NOT EXISTS idx_zd_token_txns_zero_deposit_id ON zd_token_transactions(zero_deposit_id);

-- 6e. zd_redemptions
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'zd_redemption_status') THEN
        CREATE TYPE zd_redemption_status AS ENUM (
            'draft', 'submitted', 'doc_check_pending', 'doc_check_done',
            'admin_review', 'approved', 'rejected', 'cancelled', 'completed'
        );
    END IF;
END$$;

CREATE TABLE IF NOT EXISTS zd_redemptions (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    zero_deposit_id         UUID NOT NULL REFERENCES zero_deposits(id) ON DELETE CASCADE,
    allocation_id           UUID NOT NULL REFERENCES zd_token_allocations(id),
    tokens_requested        NUMERIC(12,2) NOT NULL,
    tokens_locked           NUMERIC(12,2),
    status                  zd_redemption_status NOT NULL DEFAULT 'draft',
    property_address        TEXT,
    monthly_rent            NUMERIC(12,2),
    landlord_name           VARCHAR(255),
    landlord_phone          VARCHAR(20),
    landlord_bank_account   VARCHAR(100),
    landlord_ifsc           VARCHAR(20),
    doc_check_id            VARCHAR(255),   -- from AI doc-check service
    doc_check_flags         JSONB NOT NULL DEFAULT '[]',
    admin_remarks           TEXT,
    approved_by             UUID REFERENCES users(id) ON DELETE SET NULL,
    approved_at             TIMESTAMPTZ,
    submitted_at            TIMESTAMPTZ,
    completed_at            TIMESTAMPTZ,
    utr                     VARCHAR(100) UNIQUE,    -- unique transaction reference for payout
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Only one open (non-terminal) redemption per application
CREATE UNIQUE INDEX IF NOT EXISTS idx_zd_redemption_one_open
    ON zd_redemptions(zero_deposit_id)
    WHERE status NOT IN ('rejected', 'cancelled', 'completed');

CREATE INDEX IF NOT EXISTS idx_zd_redemptions_zero_deposit_id ON zd_redemptions(zero_deposit_id);

-- 6f. zd_redemption_documents
CREATE TABLE IF NOT EXISTS zd_redemption_documents (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    redemption_id   UUID NOT NULL REFERENCES zd_redemptions(id) ON DELETE CASCADE,
    doc_type        VARCHAR(100) NOT NULL,  -- 'agreement' | 'bank_proof' | 'landlord_kyc'
    url             TEXT NOT NULL,
    verified        BOOLEAN NOT NULL DEFAULT false,
    admin_notes     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_zd_redemption_docs_redemption_id ON zd_redemption_documents(redemption_id);

-- 6g. zd_co_applicants
CREATE TABLE IF NOT EXISTS zd_co_applicants (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    zero_deposit_id     UUID NOT NULL REFERENCES zero_deposits(id) ON DELETE CASCADE,
    name                VARCHAR(255) NOT NULL,
    phone               VARCHAR(20),
    email               VARCHAR(255),
    relationship        VARCHAR(100),
    monthly_income      NUMERIC(12,2),
    bureau_summary      JSONB NOT NULL DEFAULT '{}',
    consent_given       BOOLEAN NOT NULL DEFAULT false,
    consent_at          TIMESTAMPTZ,
    invite_expires_at   TIMESTAMPTZ,    -- 7-day invite window
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_zd_co_applicants_zero_deposit_id ON zd_co_applicants(zero_deposit_id);

-- 6h. zd_employer_verifications
CREATE TABLE IF NOT EXISTS zd_employer_verifications (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    zero_deposit_id     UUID NOT NULL REFERENCES zero_deposits(id) ON DELETE CASCADE,
    employer_name       VARCHAR(255),
    verified_name       VARCHAR(255),
    verification_source VARCHAR(100),   -- 'MCA' | 'GSTIN' | 'MANUAL'
    is_verified         BOOLEAN NOT NULL DEFAULT false,
    raw_response        JSONB NOT NULL DEFAULT '{}',
    verified_at         TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_zd_employer_verif_zero_deposit_id ON zd_employer_verifications(zero_deposit_id);

-- 6i. zd_landlord_references
CREATE TABLE IF NOT EXISTS zd_landlord_references (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    zero_deposit_id     UUID NOT NULL REFERENCES zero_deposits(id) ON DELETE CASCADE,
    landlord_name       VARCHAR(255),
    landlord_phone      VARCHAR(20),
    reference_status    VARCHAR(50) NOT NULL DEFAULT 'pending',  -- pending | positive | negative | unreachable
    contacted_at        TIMESTAMPTZ,
    notes               TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_zd_landlord_refs_zero_deposit_id ON zd_landlord_references(zero_deposit_id);

-- 6j. zd_ai_config (singleton config table)
CREATE TABLE IF NOT EXISTS zd_ai_config (
    id                              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Income multiples by employment profile
    salaried_income_multiple        NUMERIC(4,2) NOT NULL DEFAULT 3.0,
    self_employed_income_multiple   NUMERIC(4,2) NOT NULL DEFAULT 4.0,
    -- Obligation and rent caps
    obligations_cap_pct             NUMERIC(5,2) NOT NULL DEFAULT 50.0,
    rent_cap_pct                    NUMERIC(5,2) NOT NULL DEFAULT 35.0,
    -- CIBIL bands (minimum scores per tier)
    tier_a_min_cibil                INTEGER NOT NULL DEFAULT 750,
    tier_b_min_cibil                INTEGER NOT NULL DEFAULT 650,
    tier_c_min_cibil                INTEGER NOT NULL DEFAULT 600,
    -- Per-tier rent caps
    tier_a_rent_cap_pct             NUMERIC(5,2) NOT NULL DEFAULT 35.0,
    tier_b_rent_cap_pct             NUMERIC(5,2) NOT NULL DEFAULT 30.0,
    tier_c_rent_cap_pct             NUMERIC(5,2) NOT NULL DEFAULT 25.0,
    -- DPD (Days Past Due) rules
    max_dpd_12m                     INTEGER NOT NULL DEFAULT 0,
    max_dpd_24m                     INTEGER NOT NULL DEFAULT 30,
    max_enquiries_3m                INTEGER NOT NULL DEFAULT 5,
    max_bounces_6m                  INTEGER NOT NULL DEFAULT 2,
    -- Token expiry window (informational; enforced in application)
    token_validity_days             INTEGER NOT NULL DEFAULT 30,
    -- AI service connectivity
    ai_service_base_url             TEXT NOT NULL DEFAULT 'http://localhost:8001',
    ai_service_timeout_secs         INTEGER NOT NULL DEFAULT 30,
    updated_at                      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Ensure only one config row exists at all times
CREATE UNIQUE INDEX IF NOT EXISTS idx_zd_ai_config_singleton ON zd_ai_config((TRUE));

-- Seed the default config row (idempotent)
INSERT INTO zd_ai_config DEFAULT VALUES ON CONFLICT DO NOTHING;
