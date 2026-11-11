-- =============================================================================
-- Migration: Zero Deposit V2 Schema Alignment & Invariants
-- Created:   2026-11-11
-- =============================================================================

-- -----------------------------------------------------------------------------
-- 1. Align zero_deposit_ai_assessments
-- -----------------------------------------------------------------------------
ALTER TABLE zero_deposit_ai_assessments
    ADD COLUMN IF NOT EXISTS is_current                 BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN IF NOT EXISTS model_version              TEXT,
    ADD COLUMN IF NOT EXISTS decision                   VARCHAR(50),
    ADD COLUMN IF NOT EXISTS tier                       VARCHAR(10),
    ADD COLUMN IF NOT EXISTS recommended_token_amount   NUMERIC(12,2),
    ADD COLUMN IF NOT EXISTS income_considered          NUMERIC(12,2),
    ADD COLUMN IF NOT EXISTS checks                     JSONB NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS reason_codes               JSONB NOT NULL DEFAULT '[]',
    ADD COLUMN IF NOT EXISTS bureau_summary             JSONB NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS tenant_message             TEXT,
    ADD COLUMN IF NOT EXISTS override_decision          VARCHAR(50),
    ADD COLUMN IF NOT EXISTS override_reason            TEXT,
    ADD COLUMN IF NOT EXISTS overridden_by              UUID REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS overridden_at              TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS attempts                   INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS completed_at               TIMESTAMPTZ;

-- Unique current assessment per application
CREATE UNIQUE INDEX IF NOT EXISTS idx_zd_ai_assessments_one_current
    ON zero_deposit_ai_assessments(zero_deposit_id)
    WHERE is_current = true;

-- -----------------------------------------------------------------------------
-- 2. Align zd_token_allocations
-- -----------------------------------------------------------------------------
ALTER TABLE zd_token_allocations
    ADD COLUMN IF NOT EXISTS user_id         UUID REFERENCES users(id) ON DELETE CASCADE,
    ADD COLUMN IF NOT EXISTS property_id     UUID,
    ADD COLUMN IF NOT EXISTS assessment_id   UUID REFERENCES zero_deposit_ai_assessments(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS source          VARCHAR(50) NOT NULL DEFAULT 'ai_auto',
    ADD COLUMN IF NOT EXISTS issued_by       UUID REFERENCES users(id) ON DELETE SET NULL;

-- -----------------------------------------------------------------------------
-- 3. Align zd_token_transactions (append-only ledger)
-- -----------------------------------------------------------------------------
ALTER TABLE zd_token_transactions
    ADD COLUMN IF NOT EXISTS available_after NUMERIC(12,2),
    ADD COLUMN IF NOT EXISTS locked_after    NUMERIC(12,2),
    ADD COLUMN IF NOT EXISTS reference_type  TEXT,
    ADD COLUMN IF NOT EXISTS actor_type      VARCHAR(20) NOT NULL DEFAULT 'system',
    ADD COLUMN IF NOT EXISTS actor_id        UUID;

-- Positive amount constraint
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_token_txn_amount_positive'
    ) THEN
        ALTER TABLE zd_token_transactions
            ADD CONSTRAINT chk_token_txn_amount_positive CHECK (amount > 0);
    END IF;
END$$;

-- Enforce append-only with database trigger
CREATE OR REPLACE FUNCTION prevent_zd_token_transactions_update_delete()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'zd_token_transactions is append-only: updates and deletes are prohibited';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_zd_token_transactions_append_only ON zd_token_transactions;
CREATE TRIGGER trg_zd_token_transactions_append_only
BEFORE UPDATE OR DELETE ON zd_token_transactions
FOR EACH ROW EXECUTE FUNCTION prevent_zd_token_transactions_update_delete();

-- -----------------------------------------------------------------------------
-- 4. Align zd_redemptions
-- -----------------------------------------------------------------------------
ALTER TABLE zd_redemptions
    ADD COLUMN IF NOT EXISTS user_id             UUID REFERENCES users(id) ON DELETE CASCADE,
    ADD COLUMN IF NOT EXISTS chat_id             UUID REFERENCES chats(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS tokens_approved     NUMERIC(12,2),
    ADD COLUMN IF NOT EXISTS payee_snapshot      JSONB,
    ADD COLUMN IF NOT EXISTS released_by         UUID REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS released_at         TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS released_amount     NUMERIC(12,2),
    ADD COLUMN IF NOT EXISTS payout_mode         VARCHAR(50),
    ADD COLUMN IF NOT EXISTS reject_reason       TEXT,
    ADD COLUMN IF NOT EXISTS cancel_reason       TEXT,
    ADD COLUMN IF NOT EXISTS idempotency_key     TEXT;

-- Partial unique index for one open redemption per application
CREATE UNIQUE INDEX IF NOT EXISTS idx_zd_redemptions_one_open
    ON zd_redemptions(zero_deposit_id)
    WHERE status IN ('docs_pending', 'under_review', 'changes_requested', 'approved');

CREATE UNIQUE INDEX IF NOT EXISTS idx_zd_redemptions_user_idempotency
    ON zd_redemptions(user_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

-- -----------------------------------------------------------------------------
-- 5. Align zd_redemption_documents
-- -----------------------------------------------------------------------------
ALTER TABLE zd_redemption_documents
    ADD COLUMN IF NOT EXISTS label           TEXT,
    ADD COLUMN IF NOT EXISTS required        BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN IF NOT EXISTS requested_by    UUID REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS version         INTEGER NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS s3_key          TEXT,
    ADD COLUMN IF NOT EXISTS file_name       TEXT,
    ADD COLUMN IF NOT EXISTS mime_type       TEXT,
    ADD COLUMN IF NOT EXISTS size_bytes      BIGINT,
    ADD COLUMN IF NOT EXISTS status          VARCHAR(50) NOT NULL DEFAULT 'uploaded',
    ADD COLUMN IF NOT EXISTS review_comment  TEXT,
    ADD COLUMN IF NOT EXISTS reviewed_by     UUID REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS reviewed_at     TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS message_id      UUID REFERENCES messages(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS uploaded_at     TIMESTAMPTZ DEFAULT NOW();

-- -----------------------------------------------------------------------------
-- 6. Align zd_co_applicants
-- -----------------------------------------------------------------------------
ALTER TABLE zd_co_applicants
    ADD COLUMN IF NOT EXISTS user_id                 UUID REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS relation                TEXT,
    ADD COLUMN IF NOT EXISTS mobile                  TEXT,
    ADD COLUMN IF NOT EXISTS employment_type         TEXT,
    ADD COLUMN IF NOT EXISTS declared_monthly_income NUMERIC(12,2),
    ADD COLUMN IF NOT EXISTS income_considered       NUMERIC(12,2),
    ADD COLUMN IF NOT EXISTS emi_total               NUMERIC(12,2),
    ADD COLUMN IF NOT EXISTS kyc_id                  UUID REFERENCES kyc_submissions(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS cibil_score             SMALLINT,
    ADD COLUMN IF NOT EXISTS status                  VARCHAR(50) NOT NULL DEFAULT 'invited',
    ADD COLUMN IF NOT EXISTS esigned_at              TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS invited_at              TIMESTAMPTZ DEFAULT NOW(),
    ADD COLUMN IF NOT EXISTS accepted_at             TIMESTAMPTZ;

-- -----------------------------------------------------------------------------
-- 7. Align zd_employer_verifications
-- -----------------------------------------------------------------------------
ALTER TABLE zd_employer_verifications
    ADD COLUMN IF NOT EXISTS method             VARCHAR(50) NOT NULL DEFAULT 'email_otp',
    ADD COLUMN IF NOT EXISTS official_email      VARCHAR(255),
    ADD COLUMN IF NOT EXISTS otp_hash            TEXT,
    ADD COLUMN IF NOT EXISTS otp_expires_at      TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS attempts            INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS status              VARCHAR(50) NOT NULL DEFAULT 'pending',
    ADD COLUMN IF NOT EXISTS employer_category   VARCHAR(50),
    ADD COLUMN IF NOT EXISTS verified_by         UUID REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS remarks             TEXT;

-- -----------------------------------------------------------------------------
-- 8. Align zd_landlord_references
-- -----------------------------------------------------------------------------
ALTER TABLE zd_landlord_references
    ADD COLUMN IF NOT EXISTS landlord_mobile     VARCHAR(20),
    ADD COLUMN IF NOT EXISTS previous_address    TEXT,
    ADD COLUMN IF NOT EXISTS tenancy_from        DATE,
    ADD COLUMN IF NOT EXISTS tenancy_to          DATE,
    ADD COLUMN IF NOT EXISTS monthly_rent        NUMERIC(12,2),
    ADD COLUMN IF NOT EXISTS rent_receipt_urls   JSONB NOT NULL DEFAULT '[]',
    ADD COLUMN IF NOT EXISTS status              VARCHAR(50) NOT NULL DEFAULT 'pending',
    ADD COLUMN IF NOT EXISTS verified_by         UUID REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS remarks             TEXT;

-- -----------------------------------------------------------------------------
-- 9. Align zd_tenant_tiers
-- -----------------------------------------------------------------------------
ALTER TABLE zd_tenant_tiers
    ADD COLUMN IF NOT EXISTS user_id         UUID REFERENCES users(id) ON DELETE CASCADE,
    ADD COLUMN IF NOT EXISTS source          VARCHAR(50) NOT NULL DEFAULT 'assessment',
    ADD COLUMN IF NOT EXISTS on_time_months  INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS changed_by      UUID REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS reason          TEXT,
    ADD COLUMN IF NOT EXISTS updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW();

CREATE UNIQUE INDEX IF NOT EXISTS idx_zd_tenant_tiers_user_id
    ON zd_tenant_tiers(user_id)
    WHERE user_id IS NOT NULL;

-- -----------------------------------------------------------------------------
-- 10. Align zd_ai_config with exact JSON seed
-- -----------------------------------------------------------------------------
ALTER TABLE zd_ai_config
    ADD COLUMN IF NOT EXISTS config JSONB NOT NULL DEFAULT '{
        "auto_issue_enabled": true,
        "age_min": 21,
        "age_max": 55,
        "min_tenancy_months": 11,
        "deposit_rent_multiple": 3,
        "fee_flat_amount": 2000,
        "income_multiple": {"salaried":3,"self_employed":3,"gig":3.5,"student_co_applicant":3},
        "income_haircut_pct": {"self_employed":50,"gig":50,"variable_pay":50},
        "max_obligations_pct": 50,
        "max_rent_pct": 35,
        "min_cibil_score": 650,
        "co_applicant_min_cibil": 700,
        "co_applicant_window_days": 7,
        "max_enquiries_3m": 4,
        "tiers": {"A":{"min_score":750,"max_rent":40000},"B":{"min_score":700,"max_rent":25000},"C":{"min_score":650,"max_rent":15000}},
        "tier_upgrade_after_months": 6,
        "bank_checks": {"max_overdraft_days_6m":5,"max_unexplained_cash_pct_of_income":50,"max_loan_apps":2},
        "maker_checker_enabled": true,
        "redemption_required_documents": ["rental_agreement","landlord_pan","landlord_aadhaar","landlord_bank_proof","property_ownership_proof"]
    }'::jsonb;
