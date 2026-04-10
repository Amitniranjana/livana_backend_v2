-- Migration: Update carecrew_bookings and add tracking table

-- 1. Add missing columns to carecrew_bookings
ALTER TABLE carecrew_bookings
ADD COLUMN IF NOT EXISTS booking_number VARCHAR(255),
ADD COLUMN IF NOT EXISTS address TEXT,
ADD COLUMN IF NOT EXISTS problem_description TEXT,
ADD COLUMN IF NOT EXISTS contact_number VARCHAR(255),
ADD COLUMN IF NOT EXISTS estimated_cost REAL,
ADD COLUMN IF NOT EXISTS final_cost REAL,
ADD COLUMN IF NOT EXISTS payment_status VARCHAR(50) DEFAULT 'pending';

-- Generate booking_number for existing rows using a combination of date and a random hex string
UPDATE carecrew_bookings 
SET booking_number = 'BKG' || to_char(created_at, 'YYYYMMDD') || substr(md5(random()::text), 1, 6) 
WHERE booking_number IS NULL;

-- Make booking_number NOT NULL moving forward (idempotent: only if column is nullable)
DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'carecrew_bookings' AND column_name = 'booking_number' AND is_nullable = 'YES'
    ) THEN
        ALTER TABLE carecrew_bookings ALTER COLUMN booking_number SET NOT NULL;
    END IF;
END $$;

-- Add unique constraint only if not exists
DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'carecrew_bookings_booking_number_key'
    ) THEN
        ALTER TABLE carecrew_bookings ADD CONSTRAINT carecrew_bookings_booking_number_key UNIQUE (booking_number);
    END IF;
END $$;

-- 2. Create the tracking history table
CREATE TABLE IF NOT EXISTS carecrew_booking_tracking (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    booking_id UUID NOT NULL REFERENCES carecrew_bookings(id) ON DELETE CASCADE,
    status VARCHAR(50) NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed tracking status for existing bookings (only if table was just created / is empty)
INSERT INTO carecrew_booking_tracking (booking_id, status, description, created_at)
SELECT id, status, 'Booking status updated to ' || status, COALESCE(updated_at, created_at)
FROM carecrew_bookings WHERE updated_at IS NOT NULL
  AND id NOT IN (SELECT booking_id FROM carecrew_booking_tracking)
ON CONFLICT DO NOTHING;

INSERT INTO carecrew_booking_tracking (booking_id, status, description, created_at)
SELECT id, 'pending', 'Your booking has been placed successfully', created_at
FROM carecrew_bookings
WHERE id NOT IN (SELECT booking_id FROM carecrew_booking_tracking)
ON CONFLICT DO NOTHING;
