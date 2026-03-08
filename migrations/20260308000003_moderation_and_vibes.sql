-- Migration: Create tables for Moderation (Module 9) and Vibe (Module 10)

-- ═══════════════════════════════════════════════════════════════
-- 1. blocked_users — user-to-user blocking
-- ═══════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS blocked_users (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    blocker_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    blocked_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(blocker_id, blocked_id)
);

CREATE INDEX IF NOT EXISTS idx_blocked_users_blocker ON blocked_users(blocker_id);
CREATE INDEX IF NOT EXISTS idx_blocked_users_blocked ON blocked_users(blocked_id);

-- ═══════════════════════════════════════════════════════════════
-- 2. moderation_reports — entity reports (user, property, community, post)
-- ═══════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS moderation_reports (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    reporter_id   UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    entity_type   TEXT NOT NULL,         -- USER, PROPERTY, COMMUNITY, POST
    entity_id     UUID NOT NULL,
    reason        TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'PENDING_REVIEW',  -- PENDING_REVIEW, REVIEWED, DISMISSED
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_moderation_reports_reporter ON moderation_reports(reporter_id);
CREATE INDEX IF NOT EXISTS idx_moderation_reports_entity   ON moderation_reports(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_moderation_reports_status   ON moderation_reports(status);

-- ═══════════════════════════════════════════════════════════════
-- 3. archived_chats — soft-archive chats per user
-- ═══════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS archived_chats (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    chat_id       UUID NOT NULL,
    archived_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(user_id, chat_id)
);

CREATE INDEX IF NOT EXISTS idx_archived_chats_user ON archived_chats(user_id);

-- ═══════════════════════════════════════════════════════════════
-- 4. vibes — shared apartment matchmaking
-- ═══════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS vibes (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sender_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    target_user_id  UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status          TEXT NOT NULL DEFAULT 'PENDING',   -- PENDING, ACCEPTED, REJECTED
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ,

    UNIQUE(sender_id, target_user_id)
);

CREATE INDEX IF NOT EXISTS idx_vibes_sender ON vibes(sender_id);
CREATE INDEX IF NOT EXISTS idx_vibes_target ON vibes(target_user_id);
CREATE INDEX IF NOT EXISTS idx_vibes_status ON vibes(status);
