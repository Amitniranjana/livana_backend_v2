-- 20260518000001_add_listing_type_to_properties.sql

ALTER TABLE properties
ADD COLUMN IF NOT EXISTS listing_type TEXT NOT NULL DEFAULT 'Rent';

CREATE INDEX IF NOT EXISTS idx_properties_listing_type
    ON properties(listing_type);
