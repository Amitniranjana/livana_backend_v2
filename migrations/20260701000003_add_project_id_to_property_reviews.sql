-- Migration: Add project_id to property_reviews for Builder Dashboard (Module 8)
ALTER TABLE property_reviews
ADD COLUMN IF NOT EXISTS project_id UUID;
