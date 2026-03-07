-- Migration: Create community tables for Module 8
-- communities, community_members, community_posts

-- ═══════════════════════════════════════════════════════════════
-- 1. communities — community groups
-- ═══════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS communities (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name          TEXT NOT NULL,
    description   TEXT,
    created_by    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_communities_created_by  ON communities(created_by);
CREATE INDEX IF NOT EXISTS idx_communities_created_at  ON communities(created_at DESC);

-- ═══════════════════════════════════════════════════════════════
-- 2. community_members — membership tracking
-- ═══════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS community_members (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id  UUID NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    joined_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(community_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_community_members_community ON community_members(community_id);
CREATE INDEX IF NOT EXISTS idx_community_members_user      ON community_members(user_id);

-- ═══════════════════════════════════════════════════════════════
-- 3. community_posts — user posts within a community
-- ═══════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS community_posts (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id  UUID NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    author_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content       TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_community_posts_community ON community_posts(community_id);
CREATE INDEX IF NOT EXISTS idx_community_posts_author    ON community_posts(author_id);
CREATE INDEX IF NOT EXISTS idx_community_posts_created   ON community_posts(created_at DESC);
