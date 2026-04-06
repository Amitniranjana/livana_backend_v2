-- Enhance notifications table with entity reference + action support
ALTER TABLE notifications
  ADD COLUMN IF NOT EXISTS related_entity_id UUID,
  ADD COLUMN IF NOT EXISTS related_entity_type TEXT,
  ADD COLUMN IF NOT EXISTS action_status TEXT;

-- Index for filtering by notification type
CREATE INDEX IF NOT EXISTS idx_notifications_type ON notifications(type);
