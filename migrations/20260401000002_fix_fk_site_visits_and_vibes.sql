-- Fix site_visits foreign key: point property_id to listings(id) instead of properties(id)
ALTER TABLE site_visits DROP CONSTRAINT IF EXISTS site_visits_property_id_fkey;

-- Remove orphaned rows whose property_id doesn't exist in listings
DELETE FROM site_visits WHERE property_id NOT IN (SELECT id FROM listings);

ALTER TABLE site_visits ADD CONSTRAINT site_visits_property_id_fkey
  FOREIGN KEY (property_id) REFERENCES listings(id) ON DELETE CASCADE;

-- Fix vibes foreign key: point property_id to listings(id) instead of properties(id)
ALTER TABLE vibes DROP CONSTRAINT IF EXISTS vibes_property_id_fkey;

-- Remove orphaned rows whose property_id doesn't exist in listings
DELETE FROM vibes WHERE property_id IS NOT NULL AND property_id NOT IN (SELECT id FROM listings);

ALTER TABLE vibes ADD CONSTRAINT vibes_property_id_fkey
  FOREIGN KEY (property_id) REFERENCES listings(id) ON DELETE CASCADE;
