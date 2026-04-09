-- 1. Add missing fields to properties table
ALTER TABLE properties
ADD COLUMN IF NOT EXISTS likes_count INT DEFAULT 0,
ADD COLUMN IF NOT EXISTS views_count INT DEFAULT 0,
ADD COLUMN IF NOT EXISTS no_of_toilets INT DEFAULT 0,
ADD COLUMN IF NOT EXISTS no_of_balconies INT DEFAULT 0;

-- 2. Fix the property_likes foreign key constraint
-- It was mistakenly pointing to listings(id) instead of properties(id)
ALTER TABLE property_likes
DROP CONSTRAINT IF EXISTS property_likes_property_id_fkey;

-- Delete orphaned likes that point to properties that don't exist
DELETE FROM property_likes WHERE property_id NOT IN (SELECT id FROM properties);

ALTER TABLE property_likes
ADD CONSTRAINT property_likes_property_id_fkey
FOREIGN KEY (property_id) REFERENCES properties(id) ON DELETE CASCADE;
