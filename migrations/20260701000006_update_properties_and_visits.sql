-- Add project_id to properties table
ALTER TABLE properties ADD COLUMN project_id UUID REFERENCES builder_projects(id) ON DELETE SET NULL;

-- Make property_id optional in site_visits and add project_id
ALTER TABLE site_visits ALTER COLUMN property_id DROP NOT NULL;
ALTER TABLE site_visits ADD COLUMN project_id UUID REFERENCES builder_projects(id) ON DELETE CASCADE;
