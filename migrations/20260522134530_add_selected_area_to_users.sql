-- Add selected_area to users for Area-Based Notifications
ALTER TABLE users ADD COLUMN IF NOT EXISTS selected_area TEXT;
