-- Revert property_id addition to vibes (idempotent)
DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'vibes_sender_id_target_user_id_property_id_key'
    ) THEN
        ALTER TABLE vibes DROP CONSTRAINT vibes_sender_id_target_user_id_property_id_key;
    END IF;
END $$;

ALTER TABLE vibes DROP COLUMN IF EXISTS property_id;

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'vibes_sender_id_target_user_id_key'
    ) THEN
        ALTER TABLE vibes ADD CONSTRAINT vibes_sender_id_target_user_id_key UNIQUE (sender_id, target_user_id);
    END IF;
END $$;
