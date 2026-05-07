-- Add status tracking to messages table
ALTER TABLE messages
ADD COLUMN IF NOT EXISTS status VARCHAR(20) NOT NULL DEFAULT 'sent';

-- Index for querying unread messages efficiently
CREATE INDEX IF NOT EXISTS idx_messages_chat_status 
ON messages (chat_id, status);
