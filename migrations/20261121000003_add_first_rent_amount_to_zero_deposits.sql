-- -----------------------------------------------------------------------------
-- Migration: 20261121000003_add_first_rent_amount_to_zero_deposits.sql
-- Description: Add first_rent_amount column to zero_deposits table
-- -----------------------------------------------------------------------------

ALTER TABLE zero_deposits
    ADD COLUMN IF NOT EXISTS first_rent_amount NUMERIC(15,2);

