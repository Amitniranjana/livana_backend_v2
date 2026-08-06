CREATE TABLE IF NOT EXISTS builder_crm_leads (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    project_id UUID REFERENCES builder_projects(id) ON DELETE SET NULL,
    source TEXT NOT NULL,
    source_detail TEXT,
    name TEXT NOT NULL,
    phone TEXT NOT NULL,
    email TEXT,
    budget_min BIGINT,
    budget_max BIGINT,
    requirement TEXT,
    location_preference TEXT,
    status TEXT NOT NULL DEFAULT 'new',
    priority TEXT NOT NULL DEFAULT 'warm',
    notes TEXT,
    next_follow_up_date DATE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_builder_crm_leads_user_status ON builder_crm_leads(user_id, status);
CREATE INDEX IF NOT EXISTS idx_builder_crm_leads_user_source ON builder_crm_leads(user_id, source);
CREATE INDEX IF NOT EXISTS idx_builder_crm_leads_user_created_at ON builder_crm_leads(user_id, created_at DESC);
