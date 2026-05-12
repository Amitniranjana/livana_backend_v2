-- Migration: Add fields to listings_v2 and create listing_drafts table

-- 1. Add host, lease_years, and bathroom_type to listings_v2
ALTER TABLE listings_v2
ADD COLUMN IF NOT EXISTS host TEXT,
ADD COLUMN IF NOT EXISTS lease_years INTEGER,
ADD COLUMN IF NOT EXISTS bathroom_type TEXT;

-- 2. Create listing_drafts table for storing incomplete property data
CREATE TABLE IF NOT EXISTS listing_drafts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    data JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Index on user_id for faster lookups
CREATE INDEX IF NOT EXISTS idx_listing_drafts_user_id ON listing_drafts(user_id);
CREATE INDEX IF NOT EXISTS idx_listing_drafts_updated_at ON listing_drafts(updated_at DESC);
