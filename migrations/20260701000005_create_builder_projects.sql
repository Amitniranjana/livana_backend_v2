-- Create builder_projects table
CREATE TABLE IF NOT EXISTS builder_projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    project_name TEXT NOT NULL,
    project_type TEXT NOT NULL,
    status TEXT NOT NULL,
    description TEXT,
    city TEXT NOT NULL,
    locality TEXT NOT NULL,
    address TEXT NOT NULL,
    latitude FLOAT,
    longitude FLOAT,
    rera_id TEXT,
    total_units INT,
    total_towers INT,
    unit_configurations TEXT[],
    price_range_min BIGINT,
    price_range_max BIGINT,
    area_range_min_sqft INT,
    area_range_max_sqft INT,
    possession_date DATE,
    launch_date DATE,
    amenities TEXT[],
    nearby_places JSONB,
    images JSONB,
    brochure_url TEXT,
    video_url TEXT,
    master_plan_image_url TEXT,
    floor_plans JSONB,
    views_count INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create project_leads table
CREATE TABLE IF NOT EXISTS project_leads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES builder_projects(id) ON DELETE CASCADE,
    property_id UUID REFERENCES properties(id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    phone TEXT NOT NULL,
    message TEXT,
    preferred_visit_date DATE,
    status TEXT NOT NULL DEFAULT 'new',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_builder_projects_user_id ON builder_projects(user_id);
CREATE INDEX IF NOT EXISTS idx_builder_projects_city ON builder_projects(city);
CREATE INDEX IF NOT EXISTS idx_builder_projects_status ON builder_projects(status);
CREATE INDEX IF NOT EXISTS idx_project_leads_project_id ON project_leads(project_id);
