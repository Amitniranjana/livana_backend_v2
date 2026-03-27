-- Migration: Create carecrew_reviews table
CREATE TABLE IF NOT EXISTS carecrew_reviews (
    id          UUID         PRIMARY KEY DEFAULT uuid_generate_v4(),
    booking_id  UUID         NOT NULL UNIQUE,   -- 1 review per booking enforced at DB level
    provider_id UUID         NOT NULL,
    reviewer_id UUID         NOT NULL,
    rating      NUMERIC(2,1) NOT NULL CHECK (rating >= 1.0 AND rating <= 5.0),
    comment     TEXT,
    reply       TEXT,
    reply_at    TIMESTAMPTZ,
    created_at  TIMESTAMPTZ  DEFAULT NOW(),
    updated_at  TIMESTAMPTZ  DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_carecrew_reviews_provider_id ON carecrew_reviews(provider_id);
CREATE INDEX IF NOT EXISTS idx_carecrew_reviews_booking_id  ON carecrew_reviews(booking_id);
