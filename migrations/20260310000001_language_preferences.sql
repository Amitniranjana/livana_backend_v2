-- Migration: Create languages table and add preferred_language to users
-- Module 11: Language API

-- ═══════════════════════════════════════════════════════════════
-- 1. languages — available language options
-- ═══════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS languages (
    code        VARCHAR(10) PRIMARY KEY,
    name        VARCHAR(100) NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed default languages
INSERT INTO languages (code, name) VALUES
    ('en', 'English'),
    ('hi', 'Hindi'),
    ('ta', 'Tamil'),
    ('te', 'Telugu'),
    ('bn', 'Bengali'),
    ('mr', 'Marathi'),
    ('gu', 'Gujarati'),
    ('kn', 'Kannada'),
    ('ml', 'Malayalam'),
    ('pa', 'Punjabi')
ON CONFLICT (code) DO NOTHING;

-- ═══════════════════════════════════════════════════════════════
-- 2. Add preferred_language column to users table
-- ═══════════════════════════════════════════════════════════════
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS preferred_language VARCHAR(10) DEFAULT 'en';
