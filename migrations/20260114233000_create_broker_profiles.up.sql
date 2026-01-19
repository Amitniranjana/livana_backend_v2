-- Create the broker_profiles table
CREATE TABLE IF NOT EXISTS broker_profiles (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    agency_name TEXT NOT NULL,
    license_number TEXT,
    rera_id TEXT,
    operating_cities TEXT[],
    deal_types TEXT[],
    years_of_experience INT,
    kyc_status VARCHAR(50) NOT NULL DEFAULT 'PENDING',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for faster lookups (though PK user_id covers the main use case)
CREATE INDEX IF NOT EXISTS idx_broker_profiles_kyc_status ON broker_profiles(kyc_status);
