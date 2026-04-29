-- Add email and purpose columns to otp_records for email OTP support
-- This migration is purely additive — no existing columns are altered or dropped.

ALTER TABLE otp_records ADD COLUMN IF NOT EXISTS email VARCHAR(255);
ALTER TABLE otp_records ADD COLUMN IF NOT EXISTS purpose VARCHAR(50) DEFAULT 'signup';

-- Index for email-based OTP lookups
CREATE INDEX IF NOT EXISTS idx_otp_records_email ON otp_records(email);
CREATE INDEX IF NOT EXISTS idx_otp_records_email_used ON otp_records(email, used);
