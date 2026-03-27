-- Migration: Create services table for service provider listings
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE IF NOT EXISTS services (
    id           UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    provider_id  UUID        NOT NULL,
    service_name TEXT        NOT NULL,
    category     TEXT        NOT NULL,
    price        INT         NOT NULL,   -- HOURLY rate in INR
    description  TEXT        NOT NULL,
    experience   TEXT        NOT NULL,
    location     TEXT        NOT NULL,
    created_at   TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_services_provider_id ON services(provider_id);
CREATE INDEX IF NOT EXISTS idx_services_category    ON services(category);
