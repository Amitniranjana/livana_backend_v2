-- Create the listings table
CREATE TABLE IF NOT EXISTS listings (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    title VARCHAR(255) NOT NULL,
    description TEXT NOT NULL,
    city VARCHAR(100) NOT NULL,
    area VARCHAR(100),
    pincode VARCHAR(20),
    accommodation VARCHAR(50),
    apartment_type VARCHAR(50),
    roommates INT DEFAULT 0,
    gender_preference VARCHAR(50),
    carpet_area INT,
    bathrooms INT,
    price BIGINT,
    label VARCHAR(50),
    likes INT DEFAULT 0,
    host VARCHAR(255),
    is_featured BOOLEAN DEFAULT FALSE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    images JSONB DEFAULT '[]'::jsonb,
    status VARCHAR(50) DEFAULT 'active',
    views INT DEFAULT 0,
    shares INT DEFAULT 0,
    broker_commission DOUBLE PRECISION,
    is_broker_verified BOOLEAN DEFAULT FALSE,
    broker_contact_allowed BOOLEAN DEFAULT TRUE,
    priority_listing BOOLEAN DEFAULT FALSE,
    listing_type VARCHAR(50) DEFAULT 'direct',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Indexes for common search/filter fields
CREATE INDEX IF NOT EXISTS idx_listings_city ON listings(city);
CREATE INDEX IF NOT EXISTS idx_listings_area ON listings(area);
CREATE INDEX IF NOT EXISTS idx_listings_price ON listings(price);
CREATE INDEX IF NOT EXISTS idx_listings_created_at ON listings(created_at);
CREATE INDEX IF NOT EXISTS idx_listings_user_id ON listings(user_id);
