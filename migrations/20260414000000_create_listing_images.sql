CREATE TABLE IF NOT EXISTS listing_images (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    filename TEXT NOT NULL,
    size BIGINT NOT NULL,
    mime_type VARCHAR(255) NOT NULL,
    order_index INT NOT NULL DEFAULT 0,
    temp_session_id VARCHAR(255),
    listing_type VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for searching temporary session
CREATE INDEX IF NOT EXISTS idx_listing_images_session ON listing_images (temp_session_id);
-- Index for finding images per user
CREATE INDEX IF NOT EXISTS idx_listing_images_user ON listing_images (user_id);
