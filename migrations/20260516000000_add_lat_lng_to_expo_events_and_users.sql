-- Add latitude and longitude to expo_events
ALTER TABLE expo_events ADD COLUMN IF NOT EXISTS lat DOUBLE PRECISION;
ALTER TABLE expo_events ADD COLUMN IF NOT EXISTS lng DOUBLE PRECISION;

-- Add last known location for users for geo-proximity notification querying
ALTER TABLE users ADD COLUMN IF NOT EXISTS last_known_lat DOUBLE PRECISION;
ALTER TABLE users ADD COLUMN IF NOT EXISTS last_known_lng DOUBLE PRECISION;
