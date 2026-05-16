-- Add status column to job_applications
ALTER TABLE job_applications ADD COLUMN IF NOT EXISTS status VARCHAR(20) DEFAULT 'PENDING';
