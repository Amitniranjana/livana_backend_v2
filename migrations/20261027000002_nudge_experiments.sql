-- Issue 99: Personalized reminder nudge optimization (v1 — rule-based A/B)
--
-- Three tables:
--   1. nudge_experiments      — experiment definitions with variant config
--   2. nudge_variant_assignments — deterministic tenant→variant mapping (persisted)
--   3. nudge_reminder_logs    — per-send event log (ML-ready schema for v2)
--
-- Existing behavior is unchanged when no experiment is active or when the
-- NUDGE_EXPERIMENTS_ENABLED env flag is false.
--
-- Reverse migration (down):
--   DROP TABLE IF EXISTS nudge_reminder_logs;
--   DROP TABLE IF EXISTS nudge_variant_assignments;
--   DROP TABLE IF EXISTS nudge_experiments;
--   DROP TYPE  IF EXISTS nudge_channel;
--   DROP TYPE  IF EXISTS nudge_outcome;

-- ── Enums ─────────────────────────────────────────────────────────────────────

CREATE TYPE nudge_channel AS ENUM ('sms', 'push', 'call', 'email');

CREATE TYPE nudge_outcome AS ENUM (
    'pending',      -- sent, outcome not yet determined
    'delivered',    -- delivery confirmed
    'opened',       -- push/email opened or call answered
    'paid'          -- tenant made a payment within the observation window
);

-- ── 1. nudge_experiments ──────────────────────────────────────────────────────
-- Holds the experiment config. variants is a JSONB array of objects:
--   [{ "key": "control", "channel": "push", "timing_offset_hours": 0 },
--    { "key": "sms_early", "channel": "sms",  "timing_offset_hours": -24 }]

CREATE TABLE IF NOT EXISTS nudge_experiments (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name            TEXT        NOT NULL,
    description     TEXT,
    is_active       BOOLEAN     NOT NULL DEFAULT false,
    -- JSON config: array of variant objects (key, channel, timing_offset_hours, ...)
    variants        JSONB       NOT NULL DEFAULT '[]',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE nudge_experiments IS
    'A/B experiment definitions for reminder nudge optimization (Issue 99). variants is a JSONB array of variant objects each with at least {key, channel, timing_offset_hours}.';

COMMENT ON COLUMN nudge_experiments.is_active IS
    'Only active experiments are used for variant assignment. Multiple experiments can be active simultaneously (different IDs). When no experiment is active the legacy fixed-schedule path is used.';

CREATE INDEX IF NOT EXISTS idx_nudge_experiments_active
    ON nudge_experiments(is_active)
    WHERE is_active = true;

-- ── 2. nudge_variant_assignments ──────────────────────────────────────────────
-- Stores the deterministic tenant→variant assignment so it never changes after
-- first assignment. Assignment is computed as:
--   hash(tenant_id || experiment_id) % len(variants)
-- and persisted here to ensure consistency even if variant list changes.

CREATE TABLE IF NOT EXISTS nudge_variant_assignments (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    experiment_id   UUID        NOT NULL REFERENCES nudge_experiments(id) ON DELETE CASCADE,
    variant_key     TEXT        NOT NULL,
    assigned_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT uq_nudge_assignment UNIQUE (tenant_id, experiment_id)
);

COMMENT ON TABLE nudge_variant_assignments IS
    'Persisted deterministic assignment of each tenant to a variant in a given experiment. One row per (tenant, experiment) pair — immutable once written.';

CREATE INDEX IF NOT EXISTS idx_nudge_assignments_tenant
    ON nudge_variant_assignments(tenant_id);

CREATE INDEX IF NOT EXISTS idx_nudge_assignments_experiment
    ON nudge_variant_assignments(experiment_id, variant_key);

-- ── 3. nudge_reminder_logs ────────────────────────────────────────────────────
-- Every reminder sent is logged here with outcome tracking.
-- Designed so v2 ML can train directly on these rows.
-- experiment_id and variant_key are nullable to handle the legacy (no-experiment) path.

CREATE TABLE IF NOT EXISTS nudge_reminder_logs (
    id                  UUID            PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           UUID            NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    zero_deposit_id     UUID            REFERENCES zero_deposits(id) ON DELETE SET NULL,

    -- Experiment context (NULL when running in legacy mode / no active experiment)
    experiment_id       UUID            REFERENCES nudge_experiments(id) ON DELETE SET NULL,
    variant_key         TEXT,

    -- What was sent
    channel             nudge_channel   NOT NULL,
    sent_at             TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    message_preview     TEXT,       -- truncated content for audit

    -- Outcome (updated asynchronously as we receive delivery/payment events)
    outcome             nudge_outcome   NOT NULL DEFAULT 'pending',
    outcome_updated_at  TIMESTAMPTZ,

    -- Payment conversion window: set when outcome = 'paid'
    paid_within_hours   INT,        -- hours between sent_at and payment confirmed
    paid_at             TIMESTAMPTZ,

    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE nudge_reminder_logs IS
    'Append-only log of every reminder nudge sent. Outcome columns are updated asynchronously. Designed for v2 ML training on {channel, timing, variant} → payment_conversion.';

COMMENT ON COLUMN nudge_reminder_logs.paid_within_hours IS
    'Hours between sent_at and the confirmed payment. NULL if outcome != ''paid''.';

CREATE INDEX IF NOT EXISTS idx_nudge_logs_tenant
    ON nudge_reminder_logs(tenant_id, sent_at DESC);

CREATE INDEX IF NOT EXISTS idx_nudge_logs_experiment
    ON nudge_reminder_logs(experiment_id, variant_key, sent_at DESC);

CREATE INDEX IF NOT EXISTS idx_nudge_logs_outcome
    ON nudge_reminder_logs(outcome, sent_at DESC);

CREATE INDEX IF NOT EXISTS idx_nudge_logs_zero_deposit
    ON nudge_reminder_logs(zero_deposit_id);

-- Partial index for unpaid/pending rows (used by outcome-update job)
CREATE INDEX IF NOT EXISTS idx_nudge_logs_pending
    ON nudge_reminder_logs(sent_at DESC)
    WHERE outcome = 'pending';
