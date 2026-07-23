-- Admin Chat Feature Tables

CREATE TABLE IF NOT EXISTS admin_chat_threads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    admin_id VARCHAR(255),
    status VARCHAR(50) NOT NULL DEFAULT 'open', -- 'open', 'closed'
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT uq_admin_chat_user UNIQUE (user_id)
);

CREATE INDEX idx_admin_chat_threads_user ON admin_chat_threads(user_id);
CREATE INDEX idx_admin_chat_threads_admin ON admin_chat_threads(admin_id);
CREATE INDEX idx_admin_chat_threads_status ON admin_chat_threads(status);


CREATE TABLE IF NOT EXISTS admin_chat_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    thread_id UUID NOT NULL REFERENCES admin_chat_threads(id) ON DELETE CASCADE,
    sender_id VARCHAR(255) NOT NULL,
    sender_role VARCHAR(50) NOT NULL, -- 'user', 'admin'
    message TEXT NOT NULL,
    attachment_url TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_admin_chat_messages_thread ON admin_chat_messages(thread_id);
CREATE INDEX idx_admin_chat_messages_created ON admin_chat_messages(created_at ASC);


-- Property Reports Table

CREATE TABLE IF NOT EXISTS property_reports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    property_id UUID NOT NULL REFERENCES properties(id) ON DELETE CASCADE,
    reporter_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    reason TEXT NOT NULL,
    description TEXT,
    status VARCHAR(50) NOT NULL DEFAULT 'open', -- 'open', 'reviewed', 'dismissed', 'action_taken'
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT uq_property_reporter UNIQUE (property_id, reporter_id)
);

CREATE INDEX idx_property_reports_property ON property_reports(property_id);
CREATE INDEX idx_property_reports_status ON property_reports(status);
CREATE INDEX idx_property_reports_created ON property_reports(created_at DESC);
