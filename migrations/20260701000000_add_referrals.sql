-- Add referral columns to users table
ALTER TABLE users ADD COLUMN referral_code VARCHAR(12);
ALTER TABLE users ADD COLUMN referred_by_code VARCHAR(12);

-- Since referral_code must be UNIQUE and NOT NULL, we need to populate existing rows.
-- We can generate a semi-random string based on the UUID to satisfy this.
UPDATE users 
SET referral_code = UPPER(SUBSTRING(id::text, 1, 8)) 
WHERE referral_code IS NULL;

-- Now add constraints
ALTER TABLE users ALTER COLUMN referral_code SET NOT NULL;
ALTER TABLE users ADD CONSTRAINT users_referral_code_key UNIQUE (referral_code);

-- Create referrals table
CREATE TABLE referrals (
    id UUID PRIMARY KEY,
    referrer_user_id UUID NOT NULL REFERENCES users(id),
    referred_user_id UUID NOT NULL REFERENCES users(id),
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    reward_amount NUMERIC NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    rewarded_at TIMESTAMPTZ
);
