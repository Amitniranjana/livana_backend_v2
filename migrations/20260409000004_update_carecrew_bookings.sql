-- Migration: Update carecrew_bookings and add tracking table

-- 1. Add missing columns to carecrew_bookings
ALTER TABLE carecrew_bookings
ADD COLUMN IF NOT EXISTS booking_number VARCHAR(255) UNIQUE,
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

-- Make booking_number NOT NULL moving forward
ALTER TABLE carecrew_bookings ALTER COLUMN booking_number SET NOT NULL;

-- 2. Create the tracking history table
CREATE TABLE IF NOT EXISTS carecrew_booking_tracking (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    booking_id UUID NOT NULL REFERENCES carecrew_bookings(id) ON DELETE CASCADE,
    status VARCHAR(50) NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed tracking status for existing bookings
INSERT INTO carecrew_booking_tracking (booking_id, status, description, created_at)
SELECT id, status, 'Booking status updated to ' || status, updated_at
FROM carecrew_bookings WHERE updated_at IS NOT NULL
ON CONFLICT DO NOTHING;

INSERT INTO carecrew_booking_tracking (booking_id, status, description, created_at)
SELECT id, 'pending', 'Your booking has been placed successfully', created_at
FROM carecrew_bookings
ON CONFLICT DO NOTHING;
