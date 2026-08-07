-- Create pings table
CREATE TABLE IF NOT EXISTS pings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    location TEXT NOT NULL,
    latitude DOUBLE PRECISION,
    longitude DOUBLE PRECISION,
    property_type TEXT,
    listing_type TEXT,
    min_budget BIGINT,
    max_budget BIGINT,
    min_bedrooms INT,
    max_bedrooms INT,
    note TEXT,
    status TEXT NOT NULL DEFAULT 'active', -- active, closed, deleted
    close_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for broker matching feed (Issue 45)
CREATE INDEX IF NOT EXISTS idx_pings_status_location ON pings(status, location);
CREATE INDEX IF NOT EXISTS idx_pings_user_id ON pings(user_id);

-- Create ping_responses table
CREATE TABLE IF NOT EXISTS ping_responses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ping_id UUID NOT NULL REFERENCES pings(id) ON DELETE CASCADE,
    broker_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    chat_id UUID NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    message TEXT NOT NULL,
    responded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for fetching responses to a ping (Issue 47)
CREATE INDEX IF NOT EXISTS idx_ping_responses_ping_id ON ping_responses(ping_id);
CREATE INDEX IF NOT EXISTS idx_ping_responses_broker_id ON ping_responses(broker_id);
