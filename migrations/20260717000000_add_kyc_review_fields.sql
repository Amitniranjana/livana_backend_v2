ALTER TABLE kyc_submissions
ADD COLUMN reviewed_by VARCHAR(255),
ADD COLUMN reviewed_at TIMESTAMPTZ;
