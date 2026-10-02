ALTER TABLE zero_deposits DROP COLUMN IF EXISTS disbursed_at;

ALTER TABLE repayment_schedules
DROP COLUMN IF EXISTS partner_repayment_id,
DROP COLUMN IF EXISTS amount_paid,
DROP COLUMN IF EXISTS paid_at;

ALTER TABLE fee_transactions DROP COLUMN IF EXISTS upi_txn_ref;

-- Cannot easily remove an enum value in postgres, so we leave it.
