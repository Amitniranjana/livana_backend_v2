-- Migration: Create property_reviews table
CREATE TABLE IF NOT EXISTS property_reviews (
    id                  UUID         PRIMARY KEY DEFAULT uuid_generate_v4(),
    visit_id            UUID         NOT NULL UNIQUE,  -- 1 review per visit enforced at DB level
    property_id         UUID         NOT NULL,
    reviewer_id         UUID         NOT NULL,
    rating              NUMERIC(2,1) NOT NULL CHECK (rating >= 1.0 AND rating <= 5.0),
    location_rating     NUMERIC(2,1) CHECK (location_rating    >= 1.0 AND location_rating    <= 5.0),
    cleanliness_rating  NUMERIC(2,1) CHECK (cleanliness_rating >= 1.0 AND cleanliness_rating <= 5.0),
    value_rating        NUMERIC(2,1) CHECK (value_rating       >= 1.0 AND value_rating       <= 5.0),
    comment             TEXT,
    reply               TEXT,
    reply_at            TIMESTAMPTZ,
    created_at          TIMESTAMPTZ  DEFAULT NOW(),
    updated_at          TIMESTAMPTZ  DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_property_reviews_property_id ON property_reviews(property_id);
CREATE INDEX IF NOT EXISTS idx_property_reviews_visitor_id  ON property_reviews(reviewer_id);
