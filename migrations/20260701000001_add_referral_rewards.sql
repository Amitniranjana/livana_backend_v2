CREATE TABLE referral_rewards (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    referral_id UUID NOT NULL REFERENCES referrals(id),
    coupon_code VARCHAR(20) UNIQUE NOT NULL,
    amount INTEGER NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
