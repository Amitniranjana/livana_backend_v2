-- Update jobs table to match new requirements
ALTER TABLE jobs RENAME COLUMN salary_range TO salary;
ALTER TABLE jobs RENAME COLUMN associate_id TO created_by;
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS requirements TEXT;
