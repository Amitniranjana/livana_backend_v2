# Referrals API Documentation

## Overview

This documentation outlines the endpoints for the Invite & Earn referral system.

All endpoints require a valid JWT Bearer Token (`Authorization: Bearer <token>`).

---

## 1. Get My Referral Info

Retrieves the authenticated user's unique referral code, generated link, and high-level counters to display on the Referral Screen.

- **Endpoint**: `GET /api/v1/referrals/me`
- **Security**: Requires JWT

### Success Response (200 OK)

```json
{
  "success": true,
  "message": "Referral info fetched",
  "data": {
    "referral_code": "4F921034",
    "referral_link": "livana://referral?code=4F921034",
    "total_referrals": 12,
    "total_rewards_earned": 500,
    "pending_referrals": 2
  }
}
```

---

## 2. Get Referral History

Retrieves a paginated list of all users referred by the currently authenticated user. Results are sorted from newest to oldest.

- **Endpoint**: `GET /api/v1/referrals/history`
- **Security**: Requires JWT

### Success Response (200 OK)

Returns a list of referral records. The `referred_user_name` is automatically masked to protect user privacy (First Name + Last Initial).

```json
{
  "success": true,
  "message": "Referral history fetched",
  "data": {
    "referrals": [
      {
        "id": "a823f9b2-3c82-4113-90d5-7140f80bcab5",
        "referred_user_name": "Arjun K.",
        "status": "rewarded",
        "created_at": "2026-06-10T08:00:00Z"
      },
      {
        "id": "e43bdf21-7299-4c80-bdc3-4fae9b5f4922",
        "referred_user_name": "Meena R.",
        "status": "pending",
        "created_at": "2026-06-28T14:22:00Z"
      }
    ]
  }
}
```
*Note: If no referrals exist, it will successfully return an empty `referrals` array `[]`.*

---

## 3. Get Referral Rewards

Retrieves a list of all coupon codes earned by the user through successful referrals.

- **Endpoint**: `GET /api/v1/referrals/rewards`
- **Security**: Requires JWT

### Success Response (200 OK)

```json
{
  "success": true,
  "message": "Referral rewards fetched",
  "data": {
    "total_earned": 500,
    "rewards": [
      {
        "id": "871239b2-3c82-4113-90d5-7140f80bca12",
        "coupon_code": "LIV-REF-89X2PL",
        "amount": 500,
        "status": "active",
        "referred_user_name": "Arjun K.",
        "created_at": "2026-06-10T08:00:00Z",
        "expires_at": "2026-12-10T08:00:00Z"
      }
    ]
  }
}
```

---

## Edge Cases & Validation Rules

The backend natively handles the following conditions related to referrals which your frontend workflow can rely on:
- **Invalid Referral Code at Signup**: The code is silently ignored and sign-up proceeds normally. No errors are returned.
- **Self Referral**: The backend rejects attempts to use one's own code silently; the user sign-up succeeds but the referral row is not created.
- **Duplicate Referral**: The backend uses database-level unique constraints (`referred_user_id`) to prevent duplicate referrals for the same user.
- **Referred User Never Verifies OTP**: Referrals created but unverified remain in a `"pending"` status. They never produce a reward. Referrers will see them in history as pending.
- **Referrer Account Deleted**: If a referrer deletes their account, the referral row remains but no reward is issued, as the system handles it with a soft-delete mechanism on the users table.
- **Coupon Code Collision**: If a generated coupon code happens to collide with an existing one, the backend will retry generation up to 5 times before falling back and logging an error.
