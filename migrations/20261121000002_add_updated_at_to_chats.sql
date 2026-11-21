-- -----------------------------------------------------------------------------
-- Migration: 20261121000002_add_updated_at_to_chats.sql
-- Description: Add updated_at column to chats table
-- -----------------------------------------------------------------------------

ALTER TABLE chats
    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

