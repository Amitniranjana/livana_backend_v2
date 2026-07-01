# Builder Dashboard — Profile & Onboarding API Documentation

This document outlines the endpoints required for Builder Onboarding and Profile Management.

## Role Setup (Auth)

The core authentication system has been extended to support the `"builder"` role.
- During signup (`POST /api/auth/signup`), simply pass `"user_role": "builder"` to register a builder account.
- The signin flow (`POST /api/auth/signin`) remains identical. The JWT token will reflect the builder role.
- All endpoints below require a valid JWT token where the user holds the `builder` role.

---

## Base URL
`/api/builder`

---

## 1. Onboarding / Upsert Profile

Creates or completely updates the builder's company profile. This acts as an upsert (insert if not exists, update if exists).

* **Endpoint**: `POST /onboarding`
* **Headers**: `Authorization: Bearer <token>`
* **Body**:
```json
{
  "company_name": "Skyline Developers",
  "rera_id": "RERA/TN/2025/1000",
  "gst_number": "33AAACA1234A1Z5",
  "cin_number": "U70100MH2000PTC123456",
  "established_year": 2005,
  "operating_cities": ["Chennai", "Bangalore"],
  "project_categories": ["Residential", "Commercial"],
  "years_of_experience": 20,
  "total_projects_completed": 45,
  "office_address": "123 Business Park, OMR, Chennai",
  "website_url": "https://skylinedevelopers.com",
  "logo_url": "https://storage.livana.com/logo.png",
  "description": "Premium real estate developers focusing on sustainable living."
}
```

* **Success Response (200 OK)**:
```json
{
  "success": true,
  "message": "Builder profile updated successfully",
  "data": {
    "user_id": "<uuid>",
    "company_name": "Skyline Developers",
    "rera_id": "RERA/TN/2025/1000",
    "gst_number": "33AAACA1234A1Z5",
    "cin_number": "U70100MH2000PTC123456",
    "established_year": 2005,
    "operating_cities": ["Chennai", "Bangalore"],
    "project_categories": ["Residential", "Commercial"],
    "years_of_experience": 20,
    "total_projects_completed": 45,
    "office_address": "123 Business Park, OMR, Chennai",
    "website_url": "https://skylinedevelopers.com",
    "logo_url": "https://storage.livana.com/logo.png",
    "description": "Premium real estate developers focusing on sustainable living.",
    "is_verified": false,
    "created_at": "2026-07-01T10:00:00Z",
    "updated_at": "2026-07-01T10:00:00Z"
  }
}
```

---

## 2. Get Builder Profile

Fetches the profile details of the authenticated builder.

* **Endpoint**: `GET /profile`
* **Headers**: `Authorization: Bearer <token>`
* **Success Response (200 OK)**: Returns the profile data object shown above.
* **Error Response (404 Not Found)**:
```json
{
  "success": false,
  "message": "Builder profile not found",
  "error_code": "PROFILE_NOT_FOUND"
}
```

---

## 3. Update Builder Profile

Allows partial updates to the builder's profile. Any field omitted will remain unchanged.

* **Endpoint**: `PUT /profile`
* **Headers**: `Authorization: Bearer <token>`
* **Body** (All fields optional):
```json
{
  "total_projects_completed": 46,
  "description": "Updated company description."
}
```

* **Success Response (200 OK)**: Returns the fully updated profile data object.
* **Error Response (404 Not Found)**: Returned if the profile hasn't been created via `/onboarding` yet.
