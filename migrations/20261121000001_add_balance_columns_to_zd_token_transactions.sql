-- -----------------------------------------------------------------------------
-- Migration: 20261121000001_add_balance_columns_to_zd_token_transactions.sql
-- Description: Add balance_before and balance_after columns to zd_token_transactions
-- -----------------------------------------------------------------------------

ALTER TABLE zd_token_transactions
    ADD COLUMN IF NOT EXISTS balance_before NUMERIC(12,2),
    ADD COLUMN IF NOT EXISTS balance_after  NUMERIC(12,2);

