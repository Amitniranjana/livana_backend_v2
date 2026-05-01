-- Migration: Create unified listings system (listings_v2 + listing_images_v2)
-- This sits alongside the existing properties table for backward compatibility.

-- ──────────────────────────────────────────────────────────────────────────────
-- Table: listings_v2
-- ──────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS listings_v2 (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    title                 TEXT NOT NULL,
    description           TEXT NOT NULL,

    property_type         TEXT NOT NULL,      -- Residential | Commercial | Land
    listing_type          TEXT NOT NULL,      -- Rent | Sell | PG | Space Sharing
    user_type             TEXT NOT NULL,      -- User | Broker | Associate

    price                 INTEGER NOT NULL,
    deposit               INTEGER NOT NULL DEFAULT 0,

    location              TEXT NOT NULL,
    area                  TEXT,
    city                  TEXT,
    pincode               TEXT,

    latitude              DOUBLE PRECISION,
    longitude             DOUBLE PRECISION,

    area_sqft             INTEGER NOT NULL,

    bedrooms              INTEGER,
    bathrooms             INTEGER,
    no_of_toilets         INTEGER,
    no_of_balconies       INTEGER,

    furnishing            TEXT,
    facing                TEXT,

    floor                 INTEGER,
    total_floors          INTEGER,

    commercial_type       TEXT,
    land_type             TEXT,

    gender_preference     TEXT,
    roommates             INTEGER,

    amenities             TEXT[] DEFAULT '{}',
    parking               BOOLEAN DEFAULT FALSE,
    broker_contact_allowed BOOLEAN DEFAULT TRUE,

    age_years             INTEGER,

    created_by            UUID NOT NULL REFERENCES users(id),
    created_at            TIMESTAMPTZ DEFAULT NOW(),
    updated_at            TIMESTAMPTZ DEFAULT NOW()
);

-- ──────────────────────────────────────────────────────────────────────────────
-- Table: listing_images_v2
-- ──────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS listing_images_v2 (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    listing_id            UUID REFERENCES listings_v2(id) ON DELETE CASCADE,
    image_url             TEXT NOT NULL,
    display_order         INT,
    created_at            TIMESTAMPTZ DEFAULT NOW()
);

-- ──────────────────────────────────────────────────────────────────────────────
-- Indexes
-- ──────────────────────────────────────────────────────────────────────────────

CREATE INDEX IF NOT EXISTS idx_listings_v2_property_type ON listings_v2(property_type);
CREATE INDEX IF NOT EXISTS idx_listings_v2_listing_type  ON listings_v2(listing_type);
CREATE INDEX IF NOT EXISTS idx_listings_v2_user_type     ON listings_v2(user_type);
CREATE INDEX IF NOT EXISTS idx_listings_v2_city          ON listings_v2(city);
CREATE INDEX IF NOT EXISTS idx_listings_v2_price         ON listings_v2(price);
CREATE INDEX IF NOT EXISTS idx_listings_v2_location      ON listings_v2(location);
CREATE INDEX IF NOT EXISTS idx_listings_v2_created_by    ON listings_v2(created_by);
CREATE INDEX IF NOT EXISTS idx_listings_v2_created_at    ON listings_v2(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_listing_images_v2_listing ON listing_images_v2(listing_id);
CREATE INDEX IF NOT EXISTS idx_listing_images_v2_order   ON listing_images_v2(listing_id, display_order);
