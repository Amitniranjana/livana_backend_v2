-- Add down migration script here
CREATE TABLE IF NOT EXISTS users (
  id UUID PRIMARY KEY,
  first_name TEXT NOT NULL,
  last_name TEXT NOT NULL,
  email TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL,
  phone_no TEXT NOT NULL,
  gender TEXT NOT NULL,
  user_role TEXT NOT NULL,
  business_name TEXT,
  license_number TEXT,
  experience_years INTEGER,
  commission_rate DOUBLE PRECISION,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);