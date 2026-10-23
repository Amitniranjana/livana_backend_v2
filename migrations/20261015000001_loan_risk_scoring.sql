-- Issue 93: ML-based risk scoring (v1 — rule-based engine)
-- Issue 94: Bank statement cash-flow analysis
-- Issue 95: Explainable-AI decision reasons on approve/reject

-- ──────────────────────────────────────────────────────
-- 1. Bank Statement Analyses (Issue 94)
-- ──────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS bank_statement_analyses (
    id                          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    zero_deposit_id             UUID NOT NULL REFERENCES zero_deposits(id) ON DELETE CASCADE,

    -- Raw cash-flow metrics computed from the statement
    avg_monthly_credit          NUMERIC,          -- average monthly inflows
    avg_monthly_debit           NUMERIC,          -- average monthly outflows
    net_monthly_cashflow        NUMERIC,          -- avg_monthly_credit - avg_monthly_debit
    salary_credit_count         INT,              -- number of regular salary credits detected
    irregular_credit_flag       BOOLEAN NOT NULL DEFAULT FALSE,
    bounced_cheque_count        INT NOT NULL DEFAULT 0,
    high_debt_load_flag         BOOLEAN NOT NULL DEFAULT FALSE,

    -- Derived ratios
    debt_to_income_ratio        NUMERIC,          -- (existing EMIs + requested rent) / avg monthly income
    cash_flow_stability_score   NUMERIC,          -- 0-100: how regular the income is

    -- Raw flags array: ["BOUNCED_CHEQUE", "IRREGULAR_INCOME", "HIGH_DEBT_LOAD", ...]
    flags                       JSONB NOT NULL DEFAULT '[]',

    -- Analysis metadata
    statement_url               TEXT,
    analyzed_at                 TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    analysis_version            VARCHAR(20) NOT NULL DEFAULT 'v1-rule-based',

    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_bank_stmt_analyses_zero_deposit_id
    ON bank_statement_analyses(zero_deposit_id);

-- ──────────────────────────────────────────────────────
-- 2. Loan Risk Scores (Issue 93)
-- ──────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS loan_risk_scores (
    id                          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    zero_deposit_id             UUID NOT NULL REFERENCES zero_deposits(id) ON DELETE CASCADE,

    -- Composite probability-of-default score (0.0 = very safe, 1.0 = very risky)
    probability_of_default      NUMERIC(5,4) NOT NULL,

    -- Risk tier derived from PoD
    risk_tier                   VARCHAR(20) NOT NULL,   -- 'LOW' | 'MEDIUM' | 'HIGH' | 'VERY_HIGH'

    -- Sub-scores feeding into the composite (0-100 each)
    income_stability_score      NUMERIC,
    cashflow_score              NUMERIC,
    emi_burden_score            NUMERIC,

    -- Input snapshot at scoring time (for auditability / future ML retraining)
    monthly_income              NUMERIC,
    monthly_rent                NUMERIC,
    requested_deposit_amount    NUMERIC,
    existing_emi_burden         NUMERIC,
    bounced_cheque_count        INT,
    irregular_income_flag       BOOLEAN,
    high_debt_load_flag         BOOLEAN,

    -- Explainability factors (list of weighted signals, Issue 95 dependency)
    score_factors               JSONB NOT NULL DEFAULT '[]',

    -- Model metadata
    model_version               VARCHAR(20) NOT NULL DEFAULT 'v1-rule-based',
    scored_at                   TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_loan_risk_scores_zero_deposit_id
    ON loan_risk_scores(zero_deposit_id);

-- ──────────────────────────────────────────────────────
-- 3. Decision Reasons on zero_deposits (Issue 95)
--    Stores the structured explainability payload alongside
--    every admin approve / reject action (RBI Digital Lending compliance).
-- ──────────────────────────────────────────────────────
ALTER TABLE zero_deposits
    ADD COLUMN IF NOT EXISTS decision_reasons JSONB NOT NULL DEFAULT '[]',
    ADD COLUMN IF NOT EXISTS decided_by       TEXT,
    ADD COLUMN IF NOT EXISTS decided_at       TIMESTAMPTZ;

COMMENT ON COLUMN zero_deposits.decision_reasons IS
    'Structured array of {code, label, detail} objects explaining every approve/reject decision. Required by RBI Digital Lending Directions.';
