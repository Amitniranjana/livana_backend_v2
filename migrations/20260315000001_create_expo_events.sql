-- Expo Events table for Property Expo Event System
CREATE TABLE IF NOT EXISTS expo_events (
    id                UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    title             VARCHAR(255)  NOT NULL,
    description       TEXT          NOT NULL,
    location          VARCHAR(255)  NOT NULL,
    event_date        DATE          NOT NULL,
    start_time        TIME          NOT NULL,
    end_time          TIME          NOT NULL,
    organizer_id      UUID          NOT NULL,
    banner_image      VARCHAR(500)  NOT NULL DEFAULT '',
    max_participants  INTEGER       NOT NULL DEFAULT 0,
    registered_count  INTEGER       NOT NULL DEFAULT 0,
    created_at        TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_expo_events_location ON expo_events (location);
CREATE INDEX IF NOT EXISTS idx_expo_events_event_date ON expo_events (event_date);
