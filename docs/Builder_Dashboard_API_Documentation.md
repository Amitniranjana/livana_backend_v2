# Builder Dashboard API Documentation

## Overview
This document outlines the endpoints provided under the Builder Dashboard (Modules 7 and 8). 

**Authentication Required**: `Bearer <token>`  
**Role Required**: `builder`

> **Note on Infrastructure Limitations:**
> Due to the current database infrastructure, certain data points like `leads`, `views`, and `units_sold` are mocked as `0` because tracking for these metrics does not natively exist in the production backend yet. They are included in the response structure to maintain the requested frontend schema.

---

## 1. Get Dashboard Overview
Retrieves high-level analytical counters for a builder's portfolio.

- **Endpoint**: `GET /api/builder/dashboard/overview`
- **Method**: `GET`
- **Headers**: 
  - `Authorization: Bearer <token>`

### Success Response (200 OK)
```json
{
  "success": true,
  "message": "Overview fetched",
  "data": {
    "total_projects": 5,
    "active_properties": 34,
    "total_units": 0,
    "units_sold": 0,
    "total_visits": 842,
    "total_leads": 0,
    "total_views": 0,
    "profile_completion_pct": 100,
    "kyc_status": "verified"
  }
}
```

---

## 2. Get Visits Trend
Retrieves an aggregated trend of site visits over time for the builder's properties.

- **Endpoint**: `GET /api/builder/dashboard/visits-trend`
- **Method**: `GET`
- **Query Parameters**:
  - `range` (string, optional) - Intended to filter data by 7d, 30d, 12m.
  - `project_id` (string, optional) - Intended to filter data by a specific project name.
- **Headers**: 
  - `Authorization: Bearer <token>`

### Success Response (200 OK)
```json
{
  "success": true,
  "message": "Visits trend fetched",
  "data": [
    {
      "date": "2026-06-25",
      "visits": 12
    },
    {
      "date": "2026-06-26",
      "visits": 4
    }
  ]
}
```

---

## 3. Get Project Performance
Retrieves a breakdown of visits grouped by individual projects (derived via property's `project_name`).

- **Endpoint**: `GET /api/builder/dashboard/project-performance`
- **Method**: `GET`
- **Headers**: 
  - `Authorization: Bearer <token>`

### Success Response (200 OK)
```json
{
  "success": true,
  "message": "Project performance fetched",
  "data": [
    {
      "project_id": "Luxury Villas Phase 1",
      "project_name": "Luxury Villas Phase 1",
      "views": 0,
      "visits": 140,
      "leads": 0,
      "units_sold": 0,
      "units_total": 0
    }
  ]
}
```
*Note: As `project_id` does not exist structurally, the API returns the `project_name` acting as the unique identifier for the frontend.*

---

## 4. Get Top Properties
Retrieves the top-performing properties for a builder, ordered by site visit activity.

- **Endpoint**: `GET /api/builder/dashboard/top-properties`
- **Method**: `GET`
- **Headers**: 
  - `Authorization: Bearer <token>`

### Success Response (200 OK)
```json
{
  "success": true,
  "message": "Top properties fetched",
  "data": [
    {
      "id": "e43bdf21-7299-4c80-bdc3-4fae9b5f4922",
      "title": "Ocean View Apartment 4BHK",
      "visits": 54
    }
  ]
}
```

---

## 5. Reply to Property Review
Allows a builder to add a public reply to a review left on one of their own properties.

- **Endpoint**: `PATCH /api/reviews/property/{review_id}/reply`
- **Method**: `PATCH`
- **Headers**: 
  - `Authorization: Bearer <token>`
  - `Content-Type: application/json`

### Request Body
```json
{
  "reply": "Thank you for the wonderful feedback! We look forward to hosting you again."
}
```

### Success Response (201 Created)
```json
{
  "success": true,
  "message": "Reply added successfully",
  "data": {
    "review_id": "8432bdf1-6214-4c80-87a1-8bae9b5f3333",
    "reply": "Thank you for the wonderful feedback! We look forward to hosting you again.",
    "replied_at": "2026-07-01T12:00:00Z"
  }
}
```

### Errors (403 Forbidden)
Returned if the property is not owned by the authenticated builder, or if the user is not recognized as a `builder`.

```json
{
  "success": false,
  "message": "Access denied.",
  "data": null
}
```
