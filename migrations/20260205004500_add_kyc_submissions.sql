-- Add migration script here
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'kyc_doc_type') THEN
        CREATE TYPE kyc_doc_type AS ENUM ('AADHAAR', 'PAN', 'PASSPORT', 'OTHER');
    END IF;
END $$;

DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'kyc_status') THEN
        CREATE TYPE kyc_status AS ENUM ('PENDING', 'VERIFIED', 'REJECTED', 'PENDING_REVIEW');
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS kyc_submissions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    email TEXT NOT NULL,
    input_name TEXT NOT NULL,
    doc_type kyc_doc_type NOT NULL,
    s3_bucket TEXT NOT NULL,
    s3_key TEXT NOT NULL,
    file_sha256 TEXT NOT NULL,
    extracted_name TEXT,
    name_match BOOLEAN,
    status kyc_status NOT NULL DEFAULT 'PENDING',
    rejection_reason TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Index for faster lookups by user (one user might have multiple attempts)
CREATE INDEX IF NOT EXISTS idx_kyc_submissions_user_id ON kyc_submissions(user_id);
