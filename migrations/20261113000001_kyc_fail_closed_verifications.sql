-- Migration: Fail-Closed KYC Verifications, Tri-State Results, Provenance and Image Override
-- Generated: 20261113000001

ALTER TABLE kyc_submissions
    ADD COLUMN IF NOT EXISTS verification_results JSONB DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS liveness_score DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS liveness_status VARCHAR(50),
    ADD COLUMN IF NOT EXISTS face_match_reason VARCHAR(100),
    ADD COLUMN IF NOT EXISTS provenance JSONB DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS image_override_urls JSONB,
    ADD COLUMN IF NOT EXISTS image_override_audit JSONB;

-- Index for provenance / idempotency lookup
CREATE INDEX IF NOT EXISTS idx_kyc_submissions_face_match_status ON kyc_submissions(face_match_status);

-- Ensure zd_ai_config includes face_match_min_confidence if missing
UPDATE zd_ai_config
SET config = jsonb_set(
    config,
    '{face_match_min_confidence}',
    '85.0'::jsonb,
    true
)
WHERE NOT (config ? 'face_match_min_confidence');
