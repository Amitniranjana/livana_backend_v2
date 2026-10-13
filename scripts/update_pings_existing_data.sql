-- SQL Script: Standardize ping locations and clean test data
-- -----------------------------------------------------------------------------
-- Purpose:
-- 1. Standardize existing locations like 'mumbai' and 'bandra' to 'Mumbai'.
-- 2. Close or delete San Francisco test pings.
-- -----------------------------------------------------------------------------

-- 1. Standardize Mumbai and Bandra to Mumbai
UPDATE pings 
SET location = 'Mumbai', updated_at = NOW() 
WHERE lower(trim(location)) IN ('mumbai', 'bandra');

-- 2. Close or delete San Francisco test pings
UPDATE pings 
SET status = 'closed', close_reason = 'Test ping closed', updated_at = NOW() 
WHERE lower(trim(location)) LIKE '%san francisco%' OR lower(trim(location)) = 'sf';
