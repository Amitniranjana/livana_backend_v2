-- Add migration script here
CREATE TYPE zero_deposit_status AS ENUM (
    'applied',
    'pending_review',
    'blocked',
    'approved',
    'rejected'
);

CREATE TABLE zero_deposits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    property_id UUID REFERENCES properties(id) ON DELETE SET NULL,
    kyc_id UUID NOT NULL REFERENCES kyc_submissions(id) ON DELETE RESTRICT,
    monthly_rent NUMERIC(15, 2) NOT NULL,
    requested_deposit_amount NUMERIC(15, 2) NOT NULL,
    monthly_income NUMERIC(15, 2) NOT NULL,
    itr_document_url VARCHAR(512),
    bank_statement_url VARCHAR(512),
    consent_given BOOLEAN NOT NULL DEFAULT false,
    status zero_deposit_status NOT NULL DEFAULT 'applied',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_zero_deposits_user_id ON zero_deposits(user_id);
CREATE INDEX idx_zero_deposits_status ON zero_deposits(status);
