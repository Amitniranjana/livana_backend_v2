-- ============================================================
-- Migration: Google OAuth columns + Chat tables
-- ============================================================

-- 1. Extend users table with Google OAuth fields
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS google_id      TEXT UNIQUE,
    ADD COLUMN IF NOT EXISTS profile_picture TEXT;

-- 2. Chats table (represents a conversation thread)
CREATE TABLE IF NOT EXISTS chats (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name       TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 3. Chat participants (who is in which chat)
CREATE TABLE IF NOT EXISTS chat_participants (
    chat_id UUID NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (chat_id, user_id)
);

-- 4. Messages (individual messages in a chat)
CREATE TABLE IF NOT EXISTS messages (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    chat_id    UUID NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    sender_id  UUID NOT NULL REFERENCES users(id) ON DELETE SET NULL,
    content    TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 5. Index for fast per-chat message lookups (used in recent chats query)
CREATE INDEX IF NOT EXISTS idx_messages_chat_id_created_at
    ON messages (chat_id, created_at DESC);

-- 6. Index for fast participant lookup by user
CREATE INDEX IF NOT EXISTS idx_chat_participants_user_id
    ON chat_participants (user_id);
