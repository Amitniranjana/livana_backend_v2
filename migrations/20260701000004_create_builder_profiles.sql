-- Create the builder_profiles table
CREATE TABLE IF NOT EXISTS builder_profiles (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    company_name TEXT NOT NULL,
    rera_id TEXT,
    gst_number TEXT,
    cin_number TEXT,
    established_year INT,
    operating_cities TEXT[],
    project_categories TEXT[],
    years_of_experience INT,
    total_projects_completed INT,
    office_address TEXT,
    website_url TEXT,
    logo_url TEXT,
    description TEXT,
    kyc_status VARCHAR(50) NOT NULL DEFAULT 'PENDING',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for faster lookups based on KYC status
CREATE INDEX IF NOT EXISTS idx_builder_profiles_kyc_status ON builder_profiles(kyc_status);
