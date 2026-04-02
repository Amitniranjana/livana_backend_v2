# Associate Registration API Documentation

> **Version:** 1.0
> **Date:** April 2, 2026

This document details the direct `/api/v1/associates` endpoints used for registering an associate and fetching associate details. Recently updated to support optional fields based on the frontend structure.

---

## 1. Register Associate — `POST /api/v1/associates/register`

Registers a new associate user in the system. The associate's default status will be `"PENDING_KYC"`.

### Request

```http
POST /api/v1/associates/register
Content-Type: application/json
```

**Request Body Example:**
```json
{
  "name": "Jatin Krishna",
  "email": "jatink@gmail.com",
  "phone": "+917013426351",
  "password": "Secure$456",
  "associate_type": "Broker",  // (Optional)
  "gender": "male"             // (Optional)
}
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `name` | string | ✅ | User's full name. |
| `email` | string | ✅ | Email ID. |
| `phone` | string | ✅ | Mobile number (with country code preferred). |
| `password` | string | ✅ | User's plaintext password (hashed on backend). |
| `associate_type`| string| ❌ | E.g., `"Broker"`, `"Carecrew"`. If omitted, defaults to `null` in DB. |
| `gender` | string | ❌ | E.g., `"male"`, `"female"`. Defaults to `"not_specified"` if omitted. |

---

### Success Response — `201 Created`

```json
{
  "success": true,
  "message": "Associate registered successfully",
  "data": {
    "associate_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    "status": "PENDING_KYC"
  }
}
```

---

### Error Responses

| Status Code | Description |
|---|---|
| **400 Bad Request** | Missing required fields or invalid format. |
| **409 Conflict** | User with this email or phone number already exists. |
| **422 Unprocessable Entity**| Validation failed (e.g., malformed JSON payload). |
| **500 Internal Server Error**| Database error. |

---

## 2. Get Associate Profile — `GET /api/v1/associates/me`

Retrieves the currently authenticated associate's profile details. **Requires valid JWT token.**

### Request
```http
GET /api/v1/associates/me
Authorization: Bearer <JWT_Token>
```

### Success Response — `200 OK`

```json
{
  "success": true,
  "message": "Profile retrieved successfully",
  "data": {
    "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    "name": "Jatin Krishna",
    "email": "jatink@gmail.com",
    "phone": "+917013426351",
    "kbc": "Unknown",
    "associate_type": "Broker",
    "status": "PENDING_KYC",
    "created_at": "2026-04-02T10:00:00Z"
  }
}
```

---

## 3. Upload KYC Documents — `POST /api/v1/associates/{id}/kyc`

Submits documents required for background verification. Updates KYC status to pending.

### Request Body Example:
```json
{
  "aadhaar_url": "https://s3.bucket/aadhaar.pdf",
  "pan_url": "https://s3.bucket/pan.pdf",
  "business_license_url": "https://s3.bucket/license.pdf"  // (Optional)
}
```

---

## 4. Get Associate Types — `GET /api/v1/associate-types`

Fetches all available associate types from the system (e.g., Broker, Agent, Carecrew).

### Success Response Example:
```json
{
  "success": true,
  "message": "Associate types retrieved successfully",
  "data": [
    {
      "id": "e98e4f1... ",
      "name": "Agent"
    },
    {
      "id": "f51c72a... ",
      "name": "Broker"
    }
  ]
}
```
