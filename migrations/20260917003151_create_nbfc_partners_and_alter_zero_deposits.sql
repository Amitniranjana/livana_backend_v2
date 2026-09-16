-- Create enum for nbfc_status
CREATE TYPE nbfc_status AS ENUM ('active', 'inactive', 'suspended');

-- Create nbfc_partners table
CREATE TABLE IF NOT EXISTS nbfc_partners (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    entity_type VARCHAR(100) NOT NULL,
    revenue_share_pct NUMERIC(5, 2) NOT NULL DEFAULT 0.0,
    fldg_buffer_pct NUMERIC(5, 2) NOT NULL DEFAULT 0.0,
    status nbfc_status NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Alter zero_deposits table to include nbfc_id and admin approval/default tracking
ALTER TABLE zero_deposits
ADD COLUMN nbfc_id UUID REFERENCES nbfc_partners(id) ON DELETE SET NULL,
ADD COLUMN approved_deposit_amount NUMERIC(15, 2),
ADD COLUMN admin_remarks TEXT,
ADD COLUMN is_defaulted BOOLEAN NOT NULL DEFAULT false,
ADD COLUMN default_remarks TEXT;

-- Create index for nbfc_id on zero_deposits
CREATE INDEX IF NOT EXISTS idx_zero_deposits_nbfc_id ON zero_deposits(nbfc_id);
