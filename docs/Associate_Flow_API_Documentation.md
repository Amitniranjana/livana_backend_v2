# Associate User Flow — Backend API Documentation

> **Version:** 1.0
> **Date:** April 1, 2026
> **Base URL:** `https://your-server.com` (or `http://localhost:8080` for local dev)
> **Author:** Amit (Backend)

---

## Overview

This document describes the backend API endpoints that support the **Associate User Flow**. The flow allows users to register as associates, verify their phone via OTP, log in, and then select their associate sub-type (Broker or Carecrew).

### User Flow Diagram

```
Signup ──→ Send OTP ──→ Verify OTP ──→ Login ──→ Select Associate Type
  │            │             │            │              │
  POST         POST          POST         POST           PATCH
  /signup      /send-otp     /verify-otp  /signin        /associate-type
```

---

## 1. Signup — `POST /api/auth/signup`

Creates a new user account. **Does NOT send OTP automatically.** The frontend must call `/send-otp` separately after signup.

### Request

```http
POST /api/auth/signup
Content-Type: application/json
```

```json
{
  "firstName": "Arjun",
  "lastName": "Sharma",
  "email": "arjun@example.com",
  "password": "Secure@123",
  "phoneNo": "+919876543210",
  "gender": "male",
  "userRole": "associate"
}
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `firstName` | string | ✅ | |
| `lastName` | string | ✅ | |
| `email` | string | ❌ | If empty/absent, stored as empty string |
| `password` | string | ✅ | Hashed with Argon2 before storage |
| `phoneNo` | string | ✅ | Full international format: `+919876543210` |
| `gender` | string | ✅ | `"male"` / `"female"` / `"other"` |
| `userRole` | string | ✅ | **Hardcode to `"associate"`** for this flow. Defaults to `"user"` if omitted |

### Success Response — `201 Created`

```json
{
  "success": true,
  "message": "User created successfully.",
  "data": {
    "token": "eyJhbGciOiJIUzI1NiJ9...",
    "user": {
      "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      "first_name": "Arjun",
      "last_name": "Sharma",
      "email": "arjun@example.com",
      "phone_no": "+919876543210",
      "user_role": "associate",
      "verified": false,
      "is_phone_verified": false,
      "status": "active",
      "associate_type": null,
      "created_at": "2026-04-01T06:30:00Z"
    }
  }
}
```

### Error Responses

| Status | Condition |
|--------|-----------|
| `409 Conflict` | Phone number or email already exists |
| `400 Bad Request` | Missing required fields or DB error |

---

## 2. Send OTP — `POST /api/auth/send-otp`

Generates a 6-digit OTP and sends it via SMS to the provided phone number. The OTP is stored in the database with a **10-minute expiry**. If an unexpired OTP already exists for this phone, it is invalidated and replaced.

### Request

```http
POST /api/auth/send-otp
Content-Type: application/json
```

```json
{
  "phoneNo": "+919876543210"
}
```

### Success Response — `200 OK`

```json
{
  "success": true,
  "message": "OTP sent successfully",
  "data": null
}
```

### Error Responses

| Status | Condition |
|--------|-----------|
| `500 Internal Server Error` | Failed to store OTP in database |

> **Dev Note:** In development, the OTP is also printed to the server console logs for testing.

---

## 3. Verify OTP — `POST /api/auth/verify-otp`

Validates the OTP against the database. On success, marks `is_phone_verified = true` on the user record and returns a JWT + user object.

### Request

```http
POST /api/auth/verify-otp
Content-Type: application/json
```

```json
{
  "phoneNo": "+919876543210",
  "otp": "123456"
}
```

### Success Response — `200 OK`

```json
{
  "success": true,
  "message": "Phone verified successfully",
  "data": {
    "token": "eyJhbGciOiJIUzI1NiJ9...",
    "user": {
      "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      "first_name": "Arjun",
      "last_name": "Sharma",
      "email": "arjun@example.com",
      "phone_no": "+919876543210",
      "user_role": "associate",
      "associate_type": null,
      "is_phone_verified": true,
      "verified": false,
      "status": "active",
      "created_at": "2026-04-01T06:30:00Z"
    }
  }
}
```

### Error Responses

| Status | Condition |
|--------|-----------|
| `401 Unauthorized` | `"Invalid OTP"` — wrong code |
| `401 Unauthorized` | `"OTP has expired"` — older than 10 minutes |
| `401 Unauthorized` | `"No OTP found for this phone number"` — no OTP was sent |
| `404 Not Found` | No user registered with this phone number |

---

## 4. Resend OTP — `POST /api/auth/resend-otp`

Invalidates all existing OTPs for the phone number and sends a fresh one. **The 30-second cooldown is client-side only — the backend does not enforce it.**

### Request

```http
POST /api/auth/resend-otp
Content-Type: application/json
```

```json
{
  "phoneNo": "+919876543210"
}
```

### Success Response — `200 OK`

```json
{
  "success": true,
  "message": "OTP resent successfully",
  "data": null
}
```

---

## 5. Sign In — `POST /api/auth/signin`

Authenticates a user. **Now supports login via email OR phone number.** At least one must be provided.

### Request

```http
POST /api/auth/signin
Content-Type: application/json
```

**Login by email:**
```json
{
  "email": "arjun@example.com",
  "password": "Secure@123"
}
```

**Login by phone:**
```json
{
  "phoneNo": "+919876543210",
  "password": "Secure@123"
}
```

**Both provided (email takes priority):**
```json
{
  "email": "arjun@example.com",
  "phoneNo": "+919876543210",
  "password": "Secure@123"
}
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `email` | string | ❌ | At least one of `email` or `phoneNo` must be provided |
| `phoneNo` | string | ❌ | At least one of `email` or `phoneNo` must be provided |
| `password` | string | ✅ | |

### Success Response — `200 OK`

```json
{
  "success": true,
  "message": "User signed in successfully",
  "data": {
    "token": "eyJhbGciOiJIUzI1NiJ9...",
    "user": {
      "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      "first_name": "Arjun",
      "last_name": "Sharma",
      "email": "arjun@example.com",
      "phone_no": "+919876543210",
      "user_role": "associate",
      "associate_type": null,
      "is_phone_verified": true,
      "verified": false,
      "status": "active",
      "created_at": "2026-04-01T06:30:00Z"
    }
  }
}
```

### Important Fields for Frontend Routing

| Field | Values | Frontend Action |
|-------|--------|-----------------|
| `user_role` | `"associate"` | Triggers associate flow |
| `user_role` | `"user"` | Normal user flow |
| `associate_type` | `null` | First login → route to `/associate-type-selection` |
| `associate_type` | `"broker"` or `"carecrew"` | Returning user → route to `/home` |
| `is_phone_verified` | `true` / `false` | Check if phone verification is complete |

### Error Responses

| Status | Condition |
|--------|-----------|
| `400 Bad Request` | Neither email nor phoneNo provided |
| `401 Unauthorized` | Invalid credentials or account not active |

---

## 6. Update Associate Type — `PATCH /api/auth/associate-type`

Called when the associate user selects their sub-type ("broker" or "carecrew") after first login. **Requires JWT authentication.**

### Request

```http
PATCH /api/auth/associate-type
Content-Type: application/json
Authorization: Bearer <JWT_TOKEN>
```

```json
{
  "associateType": "broker"
}
```

| Field | Type | Required | Allowed Values |
|-------|------|----------|----------------|
| `associateType` | string | ✅ | `"broker"` or `"carecrew"` |

### Success Response — `200 OK`

```json
{
  "success": true,
  "message": "Associate type updated successfully",
  "data": {
    "associateType": "broker"
  }
}
```

### Error Responses

| Status | Condition |
|--------|-----------|
| `401 Unauthorized` | Missing or invalid JWT token |
| `403 Forbidden` | User's `user_role` is not `"associate"` |
| `404 Not Found` | User not found |
| `422 Unprocessable Entity` | `associateType` is not `"broker"` or `"carecrew"` |

---

## Complete Frontend Integration Flow

```
┌─────────────────────────────────────────────────────────────┐
│  STEP 1: SIGNUP                                             │
│  POST /api/auth/signup                                      │
│  Body: { firstName, lastName, phoneNo, password,            │
│          gender, userRole: "associate" }                     │
│  Save: token, user_id, user_role to SharedPreferences       │
├─────────────────────────────────────────────────────────────┤
│  STEP 2: SEND OTP                                           │
│  POST /api/auth/send-otp                                    │
│  Body: { phoneNo: "+919876543210" }                         │
│  Navigate to OTP verification screen                        │
├─────────────────────────────────────────────────────────────┤
│  STEP 3: VERIFY OTP                                         │
│  POST /api/auth/verify-otp                                  │
│  Body: { phoneNo, otp: "123456" }                           │
│  On success → navigate to login screen                      │
├─────────────────────────────────────────────────────────────┤
│  STEP 4: LOGIN                                              │
│  POST /api/auth/signin                                      │
│  Body: { email OR phoneNo, password }                       │
│  Check response: user_role + associate_type                 │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ if user_role == "associate" && associate_type == null│    │
│  │   → route to /associate-type-selection              │    │
│  │ else                                                │    │
│  │   → route to /home                                  │    │
│  └─────────────────────────────────────────────────────┘    │
├─────────────────────────────────────────────────────────────┤
│  STEP 5: SELECT ASSOCIATE TYPE                              │
│  PATCH /api/auth/associate-type                             │
│  Header: Authorization: Bearer <token>                      │
│  Body: { associateType: "broker" | "carecrew" }             │
│  On success → proceed to KYC or /home                       │
└─────────────────────────────────────────────────────────────┘
```

---

## SharedPreferences Keys Reference

These are the keys the frontend should use locally:

| Key | When to Set | Value |
|-----|-------------|-------|
| `user_type` | After signup | `"associate"` |
| `user_role` | After login | Normalized role from response |
| `user_id` / `userId` | After login | UUID from response |
| `CACHED_TOKEN` / `auth_token` | After login | JWT bearer token |
| `FIRST_LOGIN_{userId}` | After login | `false` after first redirect |
| `associate_type` | After type selection | `"broker"` or `"carecrew"` |
| `temp_password` | After login (associate only) | Raw password — **clear after type selection** |
| `onboarding_complete` | After type selection or skip | `true` |

---

## Role Normalization (Frontend)

When reading `user_role` from **any** login/signup response:

```dart
String normalizeRole(String role) {
  switch (role.toLowerCase()) {
    case 'associate':   return 'associate';
    case 'broker':
    case 'owner':       return 'broker';
    case 'carecrew':
    case 'care_crew':   return 'carecrew';
    default:
      if (role.contains('admin')) return 'admin';
      return 'user';
  }
}
```

---


