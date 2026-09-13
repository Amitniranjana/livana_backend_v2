-- Enum for mandate types
CREATE TYPE mandate_type AS ENUM ('UPI_AUTOPAY', 'NACH');

-- Enum for autopay status
CREATE TYPE autopay_status AS ENUM ('pending', 'active', 'failed');

-- Enum for repayment status
CREATE TYPE repayment_status AS ENUM ('pending', 'paid', 'overdue');

-- Enum for ledger transaction types
CREATE TYPE ledger_transaction_type AS ENUM ('disbursement', 'repayment', 'fee', 'refund');

-- Table for Autopay Mandates
CREATE TABLE IF NOT EXISTS autopay_mandates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    zero_deposit_id UUID NOT NULL REFERENCES zero_deposits(id) ON DELETE CASCADE,
    mandate_type mandate_type NOT NULL,
    upi_vpa VARCHAR(255),
    bank_account_number VARCHAR(255),
    ifsc VARCHAR(50),
    status autopay_status NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_autopay_mandates_zero_deposit_id ON autopay_mandates(zero_deposit_id);

-- Table for Repayment Schedules
CREATE TABLE IF NOT EXISTS repayment_schedules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    zero_deposit_id UUID NOT NULL REFERENCES zero_deposits(id) ON DELETE CASCADE,
    installment_number INT NOT NULL,
    due_date DATE NOT NULL,
    amount_due NUMERIC NOT NULL,
    status repayment_status NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_repayment_schedules_zero_deposit_id ON repayment_schedules(zero_deposit_id);

-- Table for Ledger Entries
CREATE TABLE IF NOT EXISTS ledger_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    zero_deposit_id UUID NOT NULL REFERENCES zero_deposits(id) ON DELETE CASCADE,
    transaction_type ledger_transaction_type NOT NULL,
    amount NUMERIC NOT NULL,
    reference_id VARCHAR(255),
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ledger_entries_zero_deposit_id ON ledger_entries(zero_deposit_id);
