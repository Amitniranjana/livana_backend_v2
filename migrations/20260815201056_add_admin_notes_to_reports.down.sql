ALTER TABLE property_reports DROP COLUMN IF EXISTS admin_notes;
ALTER TABLE moderation_reports DROP COLUMN IF EXISTS admin_notes;
ALTER TABLE moderation_reports DROP COLUMN IF EXISTS updated_at;
ALTER TABLE news_reports DROP COLUMN IF EXISTS admin_notes;
ALTER TABLE news_reports DROP COLUMN IF EXISTS updated_at;
ALTER TABLE news_reports DROP COLUMN IF EXISTS status;
