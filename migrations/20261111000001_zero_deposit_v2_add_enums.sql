-- =============================================================================
-- Migration: Zero Deposit V2 Add Enums
-- Created:   2026-11-11
-- =============================================================================

ALTER TYPE zd_redemption_status ADD VALUE IF NOT EXISTS 'docs_pending';
ALTER TYPE zd_redemption_status ADD VALUE IF NOT EXISTS 'under_review';
ALTER TYPE zd_redemption_status ADD VALUE IF NOT EXISTS 'changes_requested';
ALTER TYPE zd_redemption_status ADD VALUE IF NOT EXISTS 'released';
ALTER TYPE zd_redemption_status ADD VALUE IF NOT EXISTS 'expired';
