-- Add preferred_visit_time to project_leads
ALTER TABLE project_leads ADD COLUMN IF NOT EXISTS preferred_visit_time TIME;
