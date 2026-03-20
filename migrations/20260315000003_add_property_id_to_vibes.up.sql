TRUNCATE TABLE vibes;
ALTER TABLE vibes DROP CONSTRAINT vibes_sender_id_target_user_id_key;
ALTER TABLE vibes ADD COLUMN property_id UUID NOT NULL REFERENCES properties(id) ON DELETE CASCADE;
ALTER TABLE vibes ADD CONSTRAINT vibes_sender_id_target_user_id_property_id_key UNIQUE (sender_id, target_user_id, property_id);
