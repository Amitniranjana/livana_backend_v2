-- Migration: Create KYC submissions table (new schema matching API doc)
-- Generated: 20260228162800

-- Drop old table if it exists (only if schema was not in use)
-- DROP TABLE IF EXISTS kyc_submissions;

-- KYC Submissions Table
CREATE TABLE IF NOT EXISTS kyc_submissions (
    -- Primary
    id                          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id                     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    -- Personal Information
    full_name                   VARCHAR(255) NOT NULL,
    mobile_number               VARCHAR(20)  NOT NULL,
    email_id                    VARCHAR(255) NOT NULL,
    gender                      VARCHAR(20),
    date_of_birth               VARCHAR(20),          -- stored as string e.g. "1995-06-15"
    profile_picture_url         TEXT,

    -- Present Address
    apartment_name              VARCHAR(255),
    street_address              TEXT,
    landmark                    VARCHAR(255),
    city                        VARCHAR(100),
    zip_code                    VARCHAR(20),
    state                       VARCHAR(100),
    country                     VARCHAR(100),

    -- Permanent Address
    permanent_address           TEXT,
    is_permanent_same_as_present BOOLEAN DEFAULT FALSE,

    -- Geo
    latitude                    DOUBLE PRECISION,
    longitude                   DOUBLE PRECISION,

    -- Government ID
    govt_id_type                VARCHAR(50)  NOT NULL,   -- "aadhaar" | "pan" | "passport" | "driving_license"
    govt_id_number              VARCHAR(100) NOT NULL,
    govt_id_document_url        TEXT         NOT NULL,   -- S3 URL

    -- Professional
    company_name                VARCHAR(255),
    services                    JSONB        DEFAULT '[]',
    experience_document_url     TEXT,

    -- OCR Internal Tracking (not exposed in API responses)
    extracted_name              TEXT,
    name_match_score            DOUBLE PRECISION,

    -- Status
    verification_status         VARCHAR(30) NOT NULL DEFAULT 'pending',
                                -- "pending" | "verified" | "rejected" | "pending_review"
    rejection_reason            TEXT,

    -- Timestamps
    submitted_at                TIMESTAMPTZ DEFAULT NOW(),
    verified_at                 TIMESTAMPTZ,
    updated_at                  TIMESTAMPTZ DEFAULT NOW(),

    -- One active KYC per user
    CONSTRAINT uq_kyc_user_id UNIQUE (user_id)
);

CREATE INDEX IF NOT EXISTS idx_kyc_submissions_user_id   ON kyc_submissions(user_id);
CREATE INDEX IF NOT EXISTS idx_kyc_submissions_status    ON kyc_submissions(verification_status);
CREATE INDEX IF NOT EXISTS idx_kyc_submissions_submitted ON kyc_submissions(submitted_at DESC);

-- KYC Uploads Table (tracks files uploaded via /api/kyc/upload/*)
CREATE TABLE IF NOT EXISTS kyc_uploads (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    file_type   VARCHAR(50)  NOT NULL,   -- "profile" | "id_document" | "experience"
    s3_key      TEXT         NOT NULL,
    url         TEXT         NOT NULL,
    filename    VARCHAR(255) NOT NULL,
    size_bytes  BIGINT,
    mime_type   VARCHAR(100),
    created_at  TIMESTAMPTZ DEFAULT NOW(),
    deleted_at  TIMESTAMPTZ             -- soft delete
);

CREATE INDEX IF NOT EXISTS idx_kyc_uploads_user_id ON kyc_uploads(user_id);
