-- Create enum for nbfc_user_role
CREATE TYPE nbfc_user_role AS ENUM ('admin', 'underwriter');

-- Create enum for nbfc_user_status
CREATE TYPE nbfc_user_status AS ENUM ('active', 'inactive', 'suspended');

-- Create nbfc_users table
CREATE TABLE IF NOT EXISTS nbfc_users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    nbfc_id UUID NOT NULL REFERENCES nbfc_partners(id) ON DELETE CASCADE,
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    role nbfc_user_role NOT NULL DEFAULT 'underwriter',
    status nbfc_user_status NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for fast lookup by nbfc_id and email
CREATE INDEX IF NOT EXISTS idx_nbfc_users_nbfc_id ON nbfc_users(nbfc_id);
CREATE INDEX IF NOT EXISTS idx_nbfc_users_email ON nbfc_users(email);
