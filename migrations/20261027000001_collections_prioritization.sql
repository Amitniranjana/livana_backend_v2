-- Issue 100: Collections queue smart prioritization
-- Adds columns needed to rank defaulted zero-deposit loans by urgency score.
--
-- Reverse migration (down):
--   ALTER TABLE zero_deposits
--       DROP COLUMN IF EXISTS defaulted_at,
--       DROP COLUMN IF EXISTS outstanding_amount;
--   DROP INDEX IF EXISTS idx_zd_defaulted_at;
--   DROP INDEX IF EXISTS idx_zd_outstanding_amount;
--   DROP INDEX IF EXISTS idx_zd_is_defaulted_covering;

-- ── 1. New columns on zero_deposits ──────────────────────────────────────────

ALTER TABLE zero_deposits
    ADD COLUMN IF NOT EXISTS defaulted_at       TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS outstanding_amount NUMERIC(15, 2);

COMMENT ON COLUMN zero_deposits.defaulted_at IS
    'Timestamp when is_defaulted was set to true. Back-filled from updated_at for existing rows.';

COMMENT ON COLUMN zero_deposits.outstanding_amount IS
    'Cached outstanding balance (overdue + pending installments). Recomputed by the collections service on each query; stored for index-based filtering if needed later.';

-- Back-fill defaulted_at from updated_at for rows that are already defaulted
UPDATE zero_deposits
SET    defaulted_at = updated_at
WHERE  is_defaulted = true
  AND  defaulted_at IS NULL;

-- ── 2. Indexes used by the prioritised defaults query ─────────────────────────

-- Partial index: only rows that are actually in default (the working set for Issue 100).
CREATE INDEX IF NOT EXISTS idx_zd_is_defaulted_covering
    ON zero_deposits(is_defaulted, defaulted_at, outstanding_amount)
    WHERE is_defaulted = true;

-- Standalone indexes retained for individual column filters.
CREATE INDEX IF NOT EXISTS idx_zd_defaulted_at
    ON zero_deposits(defaulted_at)
    WHERE is_defaulted = true;

CREATE INDEX IF NOT EXISTS idx_zd_outstanding_amount
    ON zero_deposits(outstanding_amount)
    WHERE is_defaulted = true;

-- ── 3. Composite index on repayment_schedules for the JOIN ───────────────────

-- The prioritisation query groups by zero_deposit_id and filters status,
-- so a composite index on (zero_deposit_id, status, due_date) is optimal.
CREATE INDEX IF NOT EXISTS idx_rs_zd_status_due
    ON repayment_schedules(zero_deposit_id, status, due_date);
