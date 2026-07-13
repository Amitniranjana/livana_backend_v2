-- SQL Script: Link Old Associates to Brokers
-- -----------------------------------------------------------------------------
-- Purpose: 
-- Before the referral system was added, associates were not explicitly linked 
-- to their brokers in the database. This script allows you to map existing 
-- brokers to their associates using their registered phone numbers.
--
-- Instructions:
-- 1. Fill in the broker and associate phone numbers in the INSERT statement below.
-- 2. Run this script directly in your AWS RDS PostgreSQL database.
-- -----------------------------------------------------------------------------

CREATE TEMP TABLE associate_broker_mapping (
    broker_phone VARCHAR(20),
    associate_phone VARCHAR(20)
);

-- 👇 UPDATE THIS SECTION WITH YOUR ACTUAL DATA 👇
INSERT INTO associate_broker_mapping (broker_phone, associate_phone) VALUES
('+919876543210', '+919876543211'),  -- Example: Broker Phone, Associate Phone
('+919876543210', '+919876543212');  -- Add as many rows as needed
-- 👆 ----------------------------------------- 👆

-- Insert into referrals table
INSERT INTO referrals (id, referrer_user_id, referred_user_id, status, reward_amount, created_at, completed_at)
SELECT 
    uuid_generate_v4(),
    b.id, -- Broker (Referrer)
    a.id, -- Associate (Referred)
    'completed',
    0,
    NOW(),
    NOW()
FROM associate_broker_mapping m
JOIN users b ON b.phone_no = m.broker_phone
JOIN users a ON a.phone_no = m.associate_phone
WHERE b.id IS NOT NULL AND a.id IS NOT NULL
ON CONFLICT DO NOTHING; -- Prevents duplicate links if run multiple times

-- Clean up
DROP TABLE associate_broker_mapping;

SELECT 'Mapping completed successfully! Old associates are now linked.' AS status;
