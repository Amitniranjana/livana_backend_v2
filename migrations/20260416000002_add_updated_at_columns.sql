-- Migration: Add updated_at columns to tables that lack them
ALTER TABLE services ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ;
ALTER TABLE expo_events ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ;
ALTER TABLE community_posts ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ;
