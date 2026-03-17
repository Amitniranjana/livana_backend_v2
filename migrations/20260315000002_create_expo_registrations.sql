-- Expo Registrations table for tracking user registrations to expo events
CREATE TABLE IF NOT EXISTS expo_registrations (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    expo_id         UUID        NOT NULL REFERENCES expo_events(id) ON DELETE CASCADE,
    user_id         UUID        NOT NULL,
    user_type       VARCHAR(50) NOT NULL DEFAULT 'user',
    company_name    VARCHAR(255),
    registered_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Prevent duplicate registrations for same user + expo
    UNIQUE (expo_id, user_id)
);

CREATE INDEX idx_expo_registrations_expo_id ON expo_registrations (expo_id);
CREATE INDEX idx_expo_registrations_user_id ON expo_registrations (user_id);
