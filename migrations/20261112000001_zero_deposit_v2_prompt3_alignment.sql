-- -----------------------------------------------------------------------------
-- Zero Deposit v2 Prompt 3 Schema Alignment
-- Adds due_by to zd_co_applicants and ensures supporting columns exist
-- -----------------------------------------------------------------------------

ALTER TABLE zd_co_applicants
    ADD COLUMN IF NOT EXISTS due_by TIMESTAMPTZ;

-- Also support notes/remarks and admin metadata on zd_landlord_references
ALTER TABLE zd_landlord_references
    ADD COLUMN IF NOT EXISTS verified_at TIMESTAMPTZ;

