-- 1. Add new fields to the listings table
ALTER TABLE listings
ADD COLUMN IF NOT EXISTS no_of_toilets INT DEFAULT 0,
ADD COLUMN IF NOT EXISTS no_of_balconies INT DEFAULT 0;

-- 2. Create the property_likes table to track favorites
CREATE TABLE IF NOT EXISTS property_likes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    property_id UUID NOT NULL REFERENCES listings(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(user_id, property_id)
);

CREATE INDEX IF NOT EXISTS idx_property_likes_user_property ON property_likes(user_id, property_id);
