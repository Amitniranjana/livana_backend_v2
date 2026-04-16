-- Migration: Add cancellation columns to carecrew_bookings
-- Required for PUT /api/bookings/{id}/cancel endpoint

ALTER TABLE carecrew_bookings
  ADD COLUMN IF NOT EXISTS cancelled_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS cancellation_reason TEXT;

-- Index for filtering cancelled bookings
CREATE INDEX IF NOT EXISTS idx_cc_bookings_cancelled_at ON carecrew_bookings(cancelled_at)
  WHERE cancelled_at IS NOT NULL;
