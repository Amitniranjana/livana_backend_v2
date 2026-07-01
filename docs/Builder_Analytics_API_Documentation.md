# Builder Dashboard — Analytics API Documentation

This document covers the Module 7 endpoints for the Builder Dashboard Analytics. These endpoints provide aggregated statistics across a builder's properties, projects, site visits, and project leads.

## Base URL
`/api/builder/dashboard`

---

## 1. Dashboard Overview

Provides top-level KPI metrics summing data across all projects and properties owned by the authenticated builder.

* **Endpoint**: `GET /overview`
* **Headers**: `Authorization: Bearer <token>`
* **Success Response (200 OK)**:
```json
{
  "success": true,
  "message": "Overview fetched",
  "data": {
    "total_projects": 5,
    "active_properties": 23,
    "total_units": 450,
    "units_sold": 120,
    "total_visits": 310,
    "total_leads": 45,
    "total_views": 15000,
    "profile_completion_pct": 100,
    "kyc_status": "verified"
  }
}
```

---

## 2. Visits Trend

Returns a time-series array of daily visit counts across the builder's properties and projects. Can optionally be filtered down to a specific project.

* **Endpoint**: `GET /visits-trend`
* **Headers**: `Authorization: Bearer <token>`
* **Query Params**:
  * `project_id` (optional): Filter trends to a specific project's UUID.
* **Success Response (200 OK)**:
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
      "visits": 5
    }
  ]
}
```

---

## 3. Project Performance

Returns a row-by-row breakdown of performance metrics for every project the builder owns.

* **Endpoint**: `GET /project-performance`
* **Headers**: `Authorization: Bearer <token>`
* **Success Response (200 OK)**:
```json
{
  "success": true,
  "message": "Project performance fetched",
  "data": [
    {
      "project_id": "123e4567-e89b-12d3-a456-426614174000",
      "project_name": "Skyline Heights",
      "views": 4500,
      "visits": 120,
      "leads": 40,
      "units_sold": 35,
      "units_total": 240
    }
  ]
}
```

---

## 4. Top Properties

Returns a list of the builder's top 10 individual properties, ranked by total number of site visits.

* **Endpoint**: `GET /top-properties`
* **Headers**: `Authorization: Bearer <token>`
* **Success Response (200 OK)**:
```json
{
  "success": true,
  "message": "Top properties fetched",
  "data": [
    {
      "id": "223e4567-e89b-12d3-a456-426614174000",
      "title": "3BHK Premium Flat",
      "visits": 65
    }
  ]
}
```

---

## 5. Leads Trend

Returns a time-series array of daily lead counts across the builder's projects.

* **Endpoint**: `GET /leads-trend`
* **Headers**: `Authorization: Bearer <token>`
* **Query Params**:
  * `range` (optional): `7d`, `30d`, `12m`. Default is `30d`.
  * `project_id` (optional): Filter trends to a specific project.
* **Success Response (200 OK)**:
```json
{
  "success": true,
  "message": "Leads trend fetched",
  "data": [
    {
      "date": "2026-06-25",
      "leads": 4
    }
  ]
}
```

---

## 6. Views Trend

Returns a synthetic time-series of project and property page views over the requested range.

* **Endpoint**: `GET /views-trend`
* **Headers**: `Authorization: Bearer <token>`
* **Query Params**:
  * `range` (optional): `7d`, `30d`, `12m`. Default is `30d`.
  * `project_id` (optional): Filter to a specific project.
* **Success Response (200 OK)**:
```json
{
  "success": true,
  "message": "Views trend fetched",
  "data": [
    {
      "date": "2026-06-25",
      "views": 42
    }
  ]
}
```

---

## 7. Project Leads List

Returns a paginated list of all leads captured against the builder's projects. 

* **Endpoint**: `GET /leads`
* **Headers**: `Authorization: Bearer <token>`
* **Query Params**:
  * `project_id` (optional): UUID of the project.
  * `status` (optional): Filter by `new`, `contacted`, `site_visit_scheduled`, `converted`, or `dropped`.
  * `limit`, `offset` (optional)
* **Success Response (200 OK)**:
```json
{
  "success": true,
  "message": "Leads fetched",
  "data": [
    {
      "id": "111e4567-e89b-12d3-a456-426614174000",
      "project_id": "123e4567-e89b-12d3-a456-426614174000",
      "name": "Arjun Kumar",
      "phone": "9876543210",
      "message": "Interested in ground floor",
      "preferred_visit_date": "2026-07-15",
      "status": "new",
      "created_at": "2026-06-28T10:30:00Z"
    }
  ]
}
```

---

## 8. Update Lead Status

Updates the lifecycle status of a specific lead.

* **Endpoint**: `PATCH /leads/{id}/status`
* **Headers**: `Authorization: Bearer <token>`
* **Body**:
```json
{
  "status": "contacted"
}
```
* **Success Response (200 OK)**:
```json
{
  "success": true,
  "message": "Lead status updated",
  "data": {
    "id": "111e4567-e89b-12d3-a456-426614174000",
    "status": "contacted"
  }
}
```
