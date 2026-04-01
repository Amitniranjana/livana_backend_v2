-- Add is_phone_verified flag to users table
ALTER TABLE users ADD COLUMN IF NOT EXISTS is_phone_verified BOOLEAN DEFAULT FALSE;

-- OTP records table for phone verification
CREATE TABLE IF NOT EXISTS otp_records (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    phone_no VARCHAR(20) NOT NULL,
    otp_code VARCHAR(6) NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_otp_records_phone ON otp_records(phone_no);
CREATE INDEX IF NOT EXISTS idx_otp_records_phone_used ON otp_records(phone_no, used);
