CREATE TABLE IF NOT EXISTS pending_registrations (
    email VARCHAR PRIMARY KEY,
    first_name VARCHAR NOT NULL,
    last_name VARCHAR NOT NULL,
    password_hash VARCHAR NOT NULL,
    phone_no VARCHAR NOT NULL,
    gender VARCHAR NOT NULL,
    user_role VARCHAR NOT NULL DEFAULT 'user',
    business_name VARCHAR,
    license_number VARCHAR,
    experience_years INTEGER,
    commission_rate DOUBLE PRECISION,
    ref_code VARCHAR,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
