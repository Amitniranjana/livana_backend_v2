-- -----------------------------------------------------------------------------
-- Zero Deposit v2 Prompt 4 Schema Alignment: Redemptions & AI Doc Check
-- -----------------------------------------------------------------------------

-- 1. zd_ai_config: add default redemption checklist and maker-checker toggle
ALTER TABLE zd_ai_config
    ADD COLUMN IF NOT EXISTS redemption_required_documents JSONB DEFAULT '[
        {"doc_type": "rental_agreement", "label": "Signed rental agreement, 11+ months, with the deposit refund assigned to Livana", "required": true},
        {"doc_type": "landlord_pan", "label": "Landlord PAN", "required": true},
        {"doc_type": "landlord_aadhaar", "label": "Landlord Aadhaar", "required": true},
        {"doc_type": "landlord_bank_proof", "label": "Landlord cancelled cheque or passbook", "required": true},
        {"doc_type": "property_ownership_proof", "label": "Sale deed, property tax receipt or society share certificate", "required": true}
    ]',
    ADD COLUMN IF NOT EXISTS maker_checker_enabled BOOLEAN NOT NULL DEFAULT true;

-- 2. zd_redemptions: idempotency request hash & AI doc check result
ALTER TABLE zd_redemptions
    ADD COLUMN IF NOT EXISTS idempotency_request_hash TEXT,
    ADD COLUMN IF NOT EXISTS release_idempotency_key TEXT,
    ADD COLUMN IF NOT EXISTS release_idempotency_hash TEXT,
    ADD COLUMN IF NOT EXISTS ai_doc_check_result JSONB;

-- 3. zd_redemption_documents: url drop not null for missing checklist items + note column
ALTER TABLE zd_redemption_documents
    ALTER COLUMN url DROP NOT NULL;

ALTER TABLE zd_redemption_documents
    ADD COLUMN IF NOT EXISTS note TEXT;
