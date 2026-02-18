-- Add chime_user_arn column to users table
ALTER TABLE users ADD COLUMN IF NOT EXISTS chime_user_arn TEXT;
