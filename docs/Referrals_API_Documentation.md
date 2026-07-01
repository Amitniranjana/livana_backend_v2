# Referrals API Documentation

## Overview

This documentation details the `GET /api/v1/referrals/history` endpoint, used to fetch the history of users who have signed up using the logged-in user's referral code. 

## 1. Get Referral History

Retrieves a list of all referrals made by the currently authenticated user. Results are sorted from newest to oldest.

- **Endpoint**: `GET /api/v1/referrals/history`
- **Security**: Requires JWT Bearer Token (`Authentication: Bearer <token>`)

### Responses

#### 200 OK

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

#### 401 Unauthorized

Returned when the request is missing a valid JWT token.

```json
{
  "success": false,
  "message": "Unauthorized",
  "data": null
}
```

---

## Edge Cases Handled

The backend inherently handles the following conditions related to referrals which your frontend workflow can rely on:
- **Invalid Referral Code on Signup**: The code is silently ignored and sign-up proceeds normally.
- **Self Referral**: The backend blocks users attempting to refer themselves during sign-up; the user sign-up succeeds but the referral is ignored.
- **Duplicate Referral**: The backend uses database-level constraints to prevent duplicate referrals for the same user.
- **OTP Not Verified**: Referrals created but unverified simply sit in a `"pending"` state until verification happens, never producing an unearned reward.
- **Deleted Referrer**: If a referrer deletes their account before a reward is given, the platform handles it gracefully by continuing signup without producing a reward.
