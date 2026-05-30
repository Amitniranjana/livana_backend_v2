-- migrations/20260528105136_add_community_post_images.sql

ALTER TABLE community_posts 
ADD COLUMN IF NOT EXISTS images JSONB;
