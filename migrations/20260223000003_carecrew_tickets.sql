-- ─────────────────────────────────────────────────────────────────────────────
-- Migration: CareCrew Ticketing Module
-- Tables: carecrew_tickets, carecrew_ticket_comments
-- ─────────────────────────────────────────────────────────────────────────────

-- ── 1. Tickets ────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS carecrew_tickets (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Ownership
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    property_id     UUID,                               -- optional FK to a property
    assignee_id     UUID,                               -- optional: CareCrew agent assigned

    -- Content
    issue_type      VARCHAR(50)  NOT NULL,              -- e.g. 'service', 'operational', 'billing', 'other'
    description     TEXT         NOT NULL,

    -- Classification
    priority        VARCHAR(10)  NOT NULL DEFAULT 'MEDIUM'
                        CHECK (priority IN ('LOW', 'MEDIUM', 'HIGH')),

    status          VARCHAR(20)  NOT NULL DEFAULT 'OPEN'
                        CHECK (status IN ('OPEN', 'IN_PROGRESS', 'RESOLVED', 'CLOSED')),

    -- Audit
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_tickets_user_id    ON carecrew_tickets (user_id);
CREATE INDEX IF NOT EXISTS idx_tickets_status     ON carecrew_tickets (status);
CREATE INDEX IF NOT EXISTS idx_tickets_priority   ON carecrew_tickets (priority);
CREATE INDEX IF NOT EXISTS idx_tickets_assignee   ON carecrew_tickets (assignee_id);
CREATE INDEX IF NOT EXISTS idx_tickets_created_at ON carecrew_tickets (created_at DESC);

-- ── 2. Ticket Comments ────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS carecrew_ticket_comments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    ticket_id       UUID         NOT NULL REFERENCES carecrew_tickets(id) ON DELETE CASCADE,
    commenter_id    UUID         NOT NULL,              -- FK to users or agents

    comment         TEXT         NOT NULL,

    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_comments_ticket_id ON carecrew_ticket_comments (ticket_id);
CREATE INDEX IF NOT EXISTS idx_comments_created_at ON carecrew_ticket_comments (created_at ASC);

-- ── 3. Auto-update updated_at trigger ────────────────────────────────────────
CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_tickets_updated_at ON carecrew_tickets;
CREATE TRIGGER trg_tickets_updated_at
    BEFORE UPDATE ON carecrew_tickets
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
