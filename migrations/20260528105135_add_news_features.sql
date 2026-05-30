-- migrations/20260528105135_add_news_features.sql

ALTER TABLE news_items 
ADD COLUMN IF NOT EXISTS author_id UUID REFERENCES users(id),
ADD COLUMN IF NOT EXISTS status VARCHAR(20) NOT NULL DEFAULT 'pending',
ADD COLUMN IF NOT EXISTS images JSONB;

-- By default make existing news approved
UPDATE news_items SET status = 'approved' WHERE status = 'pending';

-- News Interactions
CREATE TABLE IF NOT EXISTS news_likes (
    id UUID PRIMARY KEY,
    news_id UUID NOT NULL REFERENCES news_items(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(news_id, user_id)
);

CREATE TABLE IF NOT EXISTS news_saves (
    id UUID PRIMARY KEY,
    news_id UUID NOT NULL REFERENCES news_items(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(news_id, user_id)
);

CREATE TABLE IF NOT EXISTS news_reports (
    id UUID PRIMARY KEY,
    news_id UUID NOT NULL REFERENCES news_items(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    reason TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS news_comments (
    id UUID PRIMARY KEY,
    news_id UUID NOT NULL REFERENCES news_items(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
