CREATE TABLE IF NOT EXISTS news_items (
    id UUID PRIMARY KEY,
    headline VARCHAR(255) NOT NULL,
    short_summary TEXT NOT NULL,
    source VARCHAR(255),
    category VARCHAR(100),
    published_at TIMESTAMPTZ,
    thumbnail_url VARCHAR(1024),
    views INT NOT NULL DEFAULT 0,
    clicks INT NOT NULL DEFAULT 0,
    shares INT NOT NULL DEFAULT 0,
    engagement_velocity DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    is_trending BOOLEAN NOT NULL DEFAULT FALSE,
    force_trending BOOLEAN NOT NULL DEFAULT FALSE,
    notifications_disabled BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (headline, source)
);

CREATE INDEX IF NOT EXISTS idx_news_items_trending ON news_items (is_trending) WHERE is_trending = TRUE;
CREATE INDEX IF NOT EXISTS idx_news_items_published_at ON news_items (published_at DESC);
CREATE INDEX IF NOT EXISTS idx_news_items_category ON news_items (category);
