CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    phone_no VARCHAR(20) UNIQUE NOT NULL,
    password VARCHAR(255) NOT NULL,
    gender VARCHAR(20),
    user_role VARCHAR(20) DEFAULT 'user',
    verified BOOLEAN DEFAULT FALSE,
    profile_image_url TEXT,
    bio TEXT,
    business_name TEXT,
    license_number TEXT,
    experience_years INT,
    commission_rate DOUBLE PRECISION,
    broker_rating DOUBLE PRECISION,
    total_reviews INT,
    is_verified_broker BOOLEAN DEFAULT FALSE,
    status VARCHAR(20) DEFAULT 'active',
    last_active TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);
