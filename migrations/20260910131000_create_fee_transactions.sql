-- Add fee_pending to zero_deposit_status
ALTER TYPE zero_deposit_status ADD VALUE IF NOT EXISTS 'fee_pending';

-- Create fee_transaction_status enum
CREATE TYPE fee_transaction_status AS ENUM ('pending', 'success', 'failed');

-- Create fee_transactions table
CREATE TABLE IF NOT EXISTS fee_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    zero_deposit_id UUID NOT NULL REFERENCES zero_deposits(id) ON DELETE CASCADE,
    upi_vpa VARCHAR(255) NOT NULL,
    amount NUMERIC NOT NULL,
    status fee_transaction_status NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_fee_transactions_zero_deposit_id ON fee_transactions(zero_deposit_id);
