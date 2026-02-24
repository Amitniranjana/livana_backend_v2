-- Migration: Create properties table for advanced property search
-- Run this AFTER the existing listings migration

CREATE TABLE IF NOT EXISTS properties (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title           TEXT NOT NULL,
    description     TEXT,

    -- Location fields (searchable)
    city            TEXT NOT NULL,
    locality        TEXT,
    pincode         TEXT,
    project_name    TEXT,
    builder_name    TEXT,
    landmark        TEXT,
    address         TEXT,

    -- Geo coordinates
    lat             DOUBLE PRECISION,
    lng             DOUBLE PRECISION,

    -- Property details
    property_type   TEXT NOT NULL DEFAULT 'flat',   -- flat, villa, plot, commercial
    bhk             INT,                             -- 1, 2, 3, 4+
    furnishing      TEXT DEFAULT 'unfurnished',      -- unfurnished, semi, furnished
    area_sqft       INT,
    bathrooms       INT,
    price           BIGINT,

    -- Availability
    availability    TEXT DEFAULT 'ready_to_move',   -- ready_to_move, under_construction
    possession_date DATE,

    -- Listing meta
    posted_by       TEXT DEFAULT 'owner',           -- owner, broker, builder
    user_id         UUID REFERENCES users(id) ON DELETE SET NULL,
    is_verified     BOOLEAN NOT NULL DEFAULT FALSE,
    is_featured     BOOLEAN NOT NULL DEFAULT FALSE,

    -- Rich media
    images          JSONB DEFAULT '[]',
    primary_image   TEXT,

    -- Amenities (e.g. ["lift", "parking", "gym", "swimming_pool"])
    amenities       JSONB DEFAULT '[]',

    -- Status
    status          TEXT NOT NULL DEFAULT 'active',  -- active, inactive, deleted

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ
);

-- Indexes for fast search
CREATE INDEX IF NOT EXISTS idx_properties_city         ON properties(city);
CREATE INDEX IF NOT EXISTS idx_properties_locality     ON properties(locality);
CREATE INDEX IF NOT EXISTS idx_properties_pincode      ON properties(pincode);
CREATE INDEX IF NOT EXISTS idx_properties_property_type ON properties(property_type);
CREATE INDEX IF NOT EXISTS idx_properties_bhk          ON properties(bhk);
CREATE INDEX IF NOT EXISTS idx_properties_price        ON properties(price);
CREATE INDEX IF NOT EXISTS idx_properties_status       ON properties(status);
CREATE INDEX IF NOT EXISTS idx_properties_posted_by    ON properties(posted_by);
CREATE INDEX IF NOT EXISTS idx_properties_created_at   ON properties(created_at DESC);

-- Full-text search index across key text columns
CREATE INDEX IF NOT EXISTS idx_properties_fts ON properties
    USING GIN (to_tsvector('english',
        coalesce(city, '') || ' ' ||
        coalesce(locality, '') || ' ' ||
        coalesce(project_name, '') || ' ' ||
        coalesce(builder_name, '') || ' ' ||
        coalesce(landmark, '') || ' ' ||
        coalesce(pincode, '')
    ));
