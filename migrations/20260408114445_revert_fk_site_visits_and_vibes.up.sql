-- Revert site_visits foreign key: point property_id back to properties(id) instead of listings(id)
ALTER TABLE site_visits DROP CONSTRAINT IF EXISTS site_visits_property_id_fkey;

-- Remove orphaned rows whose property_id doesn't exist in properties
DELETE FROM site_visits WHERE property_id NOT IN (SELECT id FROM properties);

ALTER TABLE site_visits ADD CONSTRAINT site_visits_property_id_fkey
  FOREIGN KEY (property_id) REFERENCES properties(id) ON DELETE CASCADE;

-- Revert vibes foreign key: point property_id back to properties(id) instead of listings(id)
ALTER TABLE vibes DROP CONSTRAINT IF EXISTS vibes_property_id_fkey;

-- Remove orphaned rows whose property_id doesn't exist in properties
DELETE FROM vibes WHERE property_id IS NOT NULL AND property_id NOT IN (SELECT id FROM properties);

ALTER TABLE vibes ADD CONSTRAINT vibes_property_id_fkey
  FOREIGN KEY (property_id) REFERENCES properties(id) ON DELETE CASCADE;
