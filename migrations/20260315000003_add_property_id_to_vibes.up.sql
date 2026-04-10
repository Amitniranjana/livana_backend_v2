-- Add property_id column to vibes table (idempotent)
DO $$ BEGIN
    -- Only drop the old constraint if it exists
    IF EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'vibes_sender_id_target_user_id_key'
    ) THEN
        ALTER TABLE vibes DROP CONSTRAINT vibes_sender_id_target_user_id_key;
    END IF;
END $$;

ALTER TABLE vibes ADD COLUMN IF NOT EXISTS property_id UUID;

-- Add FK constraint only if not exists
DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'vibes_property_id_fkey'
    ) THEN
        ALTER TABLE vibes ADD CONSTRAINT vibes_property_id_fkey
            FOREIGN KEY (property_id) REFERENCES properties(id) ON DELETE CASCADE;
    END IF;
END $$;

-- Add unique constraint only if not exists
DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'vibes_sender_id_target_user_id_property_id_key'
    ) THEN
        ALTER TABLE vibes ADD CONSTRAINT vibes_sender_id_target_user_id_property_id_key
            UNIQUE (sender_id, target_user_id, property_id);
    END IF;
END $$;
