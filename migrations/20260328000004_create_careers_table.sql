CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE IF NOT EXISTS careers (
    id              UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    title           TEXT        NOT NULL,
    description     TEXT        NOT NULL,
    location        TEXT        NOT NULL,
    employment_type TEXT        NOT NULL,
    experience      TEXT        NOT NULL,
    is_active       BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Performance indexes as required by spec
CREATE INDEX IF NOT EXISTS idx_careers_is_active  ON careers(is_active);
CREATE INDEX IF NOT EXISTS idx_careers_created_at ON careers(created_at DESC);
