-- Update jobs table to match new requirements (idempotent)

-- Rename salary_range -> salary only if old column still exists
DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'jobs' AND column_name = 'salary_range'
    ) THEN
        ALTER TABLE jobs RENAME COLUMN salary_range TO salary;
    END IF;
END $$;

-- Rename associate_id -> created_by only if old column still exists
DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'jobs' AND column_name = 'associate_id'
    ) THEN
        ALTER TABLE jobs RENAME COLUMN associate_id TO created_by;
    END IF;
END $$;

ALTER TABLE jobs ADD COLUMN IF NOT EXISTS requirements TEXT;
