-- Migration: Create CareCrew tables

CREATE TABLE IF NOT EXISTS carecrew_services (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL,
    description TEXT,
    icon_url    TEXT,
    category    TEXT,
    is_active   BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS carecrew_providers (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            TEXT NOT NULL,
    bio             TEXT,
    service_type    TEXT NOT NULL,  -- e.g. plumbing, electrical, cleaning, painting
    city            TEXT,
    rating          REAL DEFAULT 0.0,
    review_count    INT DEFAULT 0,
    is_featured     BOOLEAN NOT NULL DEFAULT FALSE,
    avatar_url      TEXT,
    phone           TEXT,
    user_id         UUID REFERENCES users(id) ON DELETE SET NULL,
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS carecrew_bookings (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_id     UUID NOT NULL REFERENCES carecrew_providers(id) ON DELETE CASCADE,
    service_id      UUID NOT NULL REFERENCES carecrew_services(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    scheduled_at    TIMESTAMPTZ NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending',  -- pending, confirmed, in_progress, completed, cancelled
    notes           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_cc_providers_service_type ON carecrew_providers(service_type);
CREATE INDEX IF NOT EXISTS idx_cc_providers_city         ON carecrew_providers(city);
CREATE INDEX IF NOT EXISTS idx_cc_providers_featured     ON carecrew_providers(is_featured);
CREATE INDEX IF NOT EXISTS idx_cc_bookings_user_id       ON carecrew_bookings(user_id);
CREATE INDEX IF NOT EXISTS idx_cc_bookings_provider_id   ON carecrew_bookings(provider_id);
CREATE INDEX IF NOT EXISTS idx_cc_bookings_status        ON carecrew_bookings(status);

-- Seed some default services
INSERT INTO carecrew_services (name, description, icon_url, category, is_active) VALUES
    ('Plumbing',         'Pipe repairs, leak fixes, installations',           '/icons/plumbing.svg',     'Home Repair', TRUE),
    ('Electrical',       'Wiring, switches, appliance repairs',               '/icons/electrical.svg',   'Home Repair', TRUE),
    ('Deep Cleaning',    'Full home deep cleaning service',                   '/icons/cleaning.svg',     'Cleaning',    TRUE),
    ('Painting',         'Interior & exterior painting',                      '/icons/painting.svg',     'Home Repair', TRUE),
    ('Carpentry',        'Furniture assembly, woodwork repairs',              '/icons/carpentry.svg',    'Home Repair', TRUE),
    ('Pest Control',     'Spray treatments for cockroaches, ants, etc.',     '/icons/pest.svg',         'Hygiene',     TRUE),
    ('AC Service',       'AC cleaning, gas refill, maintenance',             '/icons/ac.svg',           'Appliances',  TRUE),
    ('Appliance Repair', 'Washing machine, fridge, microwave repairs',       '/icons/appliance.svg',    'Appliances',  TRUE)
ON CONFLICT DO NOTHING;
