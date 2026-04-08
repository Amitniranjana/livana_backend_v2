-- Add message_type to messages table to support media (image/document) messages
ALTER TABLE messages
    ADD COLUMN IF NOT EXISTS message_type TEXT NOT NULL DEFAULT 'text';

-- Index for filtering by type if needed
CREATE INDEX IF NOT EXISTS idx_messages_chat_id_type
    ON messages (chat_id, message_type);
