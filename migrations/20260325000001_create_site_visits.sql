-- Site visit bookings table
CREATE TABLE site_visits (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    property_id         UUID NOT NULL REFERENCES properties(id) ON DELETE CASCADE,
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    scheduled_date_time TIMESTAMPTZ NOT NULL,
    status              VARCHAR(20) NOT NULL DEFAULT 'pending',
    address             TEXT,
    contact_number      VARCHAR(20),
    notes               TEXT,
    cancellation_reason TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Status values: pending, confirmed, completed, cancelled
    CONSTRAINT valid_status CHECK (
        status IN ('pending', 'confirmed', 'completed', 'cancelled')
    )
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_site_visits_user_id
    ON site_visits(user_id);

CREATE INDEX IF NOT EXISTS idx_site_visits_provider_id
    ON site_visits(provider_id);

CREATE INDEX IF NOT EXISTS idx_site_visits_property_id
    ON site_visits(property_id);

CREATE INDEX IF NOT EXISTS idx_site_visits_scheduled
    ON site_visits(scheduled_date_time);

-- Duplicate booking prevent karne ke liye
-- Same user, same property, same time pe dobara book nahi kar sakta
CREATE UNIQUE INDEX IF NOT EXISTS idx_visits_no_duplicate
    ON site_visits(property_id, user_id, scheduled_date_time)
    WHERE status NOT IN ('cancelled');
