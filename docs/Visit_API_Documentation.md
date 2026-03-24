# 🏠 Site Visit Booking API Documentation

> **Base URL:** `http://localhost:9090` (dev)
> **Auth:** All endpoints require `Authorization: Bearer <jwt_token>` 🔒

---

## Table of Contents
1. [Book a Site Visit](#1-book-a-site-visit)
2. [Get All Visits](#2-get-all-visits)
3. [Get Visit Detail](#3-get-visit-detail)
4. [Update Visit Status](#4-update-visit-status)
5. [Status Flow](#status-flow)
6. [Error Codes](#error-codes)

---

## 1. Book a Site Visit

### `POST /api/visits` 🔒

Books a new site visit for the authenticated user on a specific property.

#### Request Body
```json
{
  "property_id": "uuid",
  "provider_id": "uuid",
  "scheduled_date_time": "2026-04-01T10:00:00Z",
  "contact_number": "+919876543210",
  "notes": "Please call before arriving"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `property_id` | UUID | ✅ | Property to visit |
| `provider_id` | UUID | ✅ | Provider/broker who will conduct the visit |
| `scheduled_date_time` | ISO 8601 | ✅ | Date and time for the visit |
| `contact_number` | string | ✅ | Contact number for the visit |
| `notes` | string | ❌ | Additional notes/instructions |

#### Success Response `201 Created`
```json
{
  "success": true,
  "message": "Visit booked successfully",
  "data": {
    "visit_id": "uuid",
    "property": {
      "id": "uuid",
      "title": "3 BHK Flat in Sector 62",
      "location": "Sector 62"
    },
    "provider": {
      "id": "uuid",
      "name": "Rahul Sharma",
      "profile_image": "https://cdn.example.com/photo.jpg"
    },
    "scheduled_date_time": "2026-04-01T10:00:00Z",
    "status": "pending",
    "contact_number": "+919876543210",
    "notes": "Please call before arriving",
    "cancellation_reason": null,
    "created_at": "2026-03-25T00:00:00Z"
  }
}
```

#### Error Responses
| Code | HTTP | When |
|------|------|------|
| `PROPERTY_NOT_FOUND` | 404 | Property ID does not exist |
| `VISIT_ALREADY_EXISTS` | 409 | Duplicate booking — same user + property + time |
| `INVALID_TOKEN` | 401 | Missing/invalid JWT |
| `DATABASE_ERROR` | 500 | DB failure |

---

## 2. Get All Visits

### `GET /api/visits` 🔒

Returns all visits booked by the authenticated user, sorted by scheduled date (newest first).

#### Success Response `200 OK`
```json
{
  "success": true,
  "data": [
    {
      "visit_id": "uuid",
      "property": { "id": "uuid", "title": "...", "location": "..." },
      "provider": { "id": "uuid", "name": "...", "profile_image": "..." },
      "scheduled_date_time": "2026-04-01T10:00:00Z",
      "status": "pending",
      "contact_number": "+919876543210",
      "notes": null,
      "cancellation_reason": null,
      "created_at": "2026-03-25T00:00:00Z"
    }
  ]
}
```

> **Note:** Returns `"data": []` (empty array) if user has no visits — not an error.

---

## 3. Get Visit Detail

### `GET /api/visits/{visit_id}` 🔒

Returns details of a single visit. Only accessible by the visit's **user** or **provider**.

#### Success Response `200 OK`
```json
{
  "success": true,
  "data": {
    "visit_id": "uuid",
    "property": { "id": "uuid", "title": "...", "location": "..." },
    "provider": { "id": "uuid", "name": "...", "profile_image": "..." },
    "scheduled_date_time": "2026-04-01T10:00:00Z",
    "status": "confirmed",
    "contact_number": "+919876543210",
    "notes": "Morning preferred",
    "cancellation_reason": null,
    "created_at": "2026-03-25T00:00:00Z"
  }
}
```

#### Error Responses
| Code | HTTP | When |
|------|------|------|
| `VISIT_NOT_FOUND` | 404 | Visit doesn't exist or user is not a participant |
| `INVALID_TOKEN` | 401 | Missing/invalid JWT |

---

## 4. Update Visit Status

### `PUT /api/visits/{visit_id}/status` 🔒

Updates the status of a visit. **Only the assigned provider** can update the status.

#### Request Body
```json
{
  "status": "confirmed",
  "cancellation_reason": null
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `status` | string | ✅ | `confirmed` \| `completed` \| `cancelled` |
| `cancellation_reason` | string | ❌ | Required if status is `cancelled` |

#### Success Response `200 OK`
```json
{
  "success": true,
  "message": "Visit status updated to 'confirmed'"
}
```

#### Error Responses
| Code | HTTP | When |
|------|------|------|
| `INVALID_STATUS` | 400 | Status not in allowed list |
| `ACCESS_DENIED` | 403 | User is not the provider for this visit |
| `INVALID_TOKEN` | 401 | Missing/invalid JWT |
| `DATABASE_ERROR` | 500 | DB failure |

---

## Status Flow

```
pending ──► confirmed ──► completed
  │              │
  ▼              ▼
cancelled    cancelled
```

> ⚠️ Once a visit is `completed` or `cancelled`, it cannot be changed.

---

## Error Codes

| Error Code | HTTP | Description |
|------------|------|-------------|
| `INVALID_TOKEN` | 401 | Missing or invalid Bearer token |
| `PROPERTY_NOT_FOUND` | 404 | Property UUID doesn't exist |
| `VISIT_ALREADY_EXISTS` | 409 | Duplicate booking (same user + property + time) |
| `VISIT_NOT_FOUND` | 404 | Visit doesn't exist or not accessible |
| `INVALID_STATUS` | 400 | Status value not in allowed list |
| `ACCESS_DENIED` | 403 | User is not the provider for that visit |
| `DATABASE_ERROR` | 500 | General database error |

---

## Quick Reference

| Method | Endpoint | Auth | Description |
|--------|----------|------|-------------|
| POST | `/api/visits` | 🔒 | Book a site visit |
| GET | `/api/visits` | 🔒 | Get all user visits |
| GET | `/api/visits/{visit_id}` | 🔒 | Get single visit detail |
| PUT | `/api/visits/{visit_id}/status` | 🔒 | Update visit status (provider only) |
