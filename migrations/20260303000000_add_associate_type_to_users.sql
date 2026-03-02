-- Add associate_type to users table
ALTER TABLE users ADD COLUMN IF NOT EXISTS associate_type VARCHAR(50) NULL;
