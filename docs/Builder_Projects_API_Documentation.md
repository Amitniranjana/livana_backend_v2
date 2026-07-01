# Builder Dashboard — Projects & Visits API Documentation

This document outlines the endpoints required for the Builder Dashboard "Projects" module, as well as the newly updated site visits engine.

## Base URL
`/api`

---

## 1. Builder Projects (Authenticated, Role = Builder)

### 1.1 Create Project
Creates a new macro project (e.g., a society, a gated community, or an apartment tower).
* **Endpoint**: `POST /builder/projects`
* **Headers**: `Authorization: Bearer <token>`
* **Body**:
```json
{
  "project_name": "Skyline Heights",
  "project_type": "Residential",
  "status": "upcoming",            
  "description": "Premium 2/3 BHK apartments with clubhouse.",
  "city": "Chennai",
  "locality": "OMR",
  "address": "Near ELCOT SEZ, OMR",
  "latitude": 12.9010,
  "longitude": 80.2270,
  "rera_id": "RERA/TN/2025/000456",
  "total_units": 240,
  "total_towers": 3,
  "unit_configurations": ["2BHK", "3BHK", "4BHK"],
  "price_range_min": 6500000,
  "price_range_max": 15000000,
  "area_range_min_sqft": 950,
  "area_range_max_sqft": 2100,
  "possession_date": "2027-06-30",
  "launch_date": "2025-01-15",
  "amenities": ["Clubhouse", "Swimming Pool", "Gym", "Kids Play Area"],
  "nearby_places": {"schools": ["DAV"], "hospitals": ["Apollo"]},
  "images": ["https://.../1.jpg", "https://.../2.jpg"],
  "brochure_url": "https://.../brochure.pdf",
  "video_url": "https://.../walkthrough.mp4",
  "master_plan_image_url": "https://.../masterplan.jpg",
  "floor_plans": [
    {"type": "2BHK", "area_sqft": 1050, "price": 6500000, "image_url": "https://.../2bhk.jpg"}
  ]
}
```

### 1.2 Get Builder's Projects
Fetch paginated projects owned by the authenticated builder, enriched with performance metrics.
* **Endpoint**: `GET /builder/projects`
* **Query Params**:
  * `status` (optional): `upcoming`, `ongoing`, `completed`, or `all`.
  * `limit` (optional, default 20).
  * `offset` (optional, default 0).
* **Response**: Returns a list of projects and pagination metadata.
```json
{
  "success": true,
  "data": {
    "projects": [
      {
        "id": "...",
        "project_name": "Skyline Heights",
        "units_sold": 15,
        "visits_count": 42,
        "leads_count": 10
        // ... standard project fields
      }
    ],
    "pagination": { "total": 1, "limit": 20, "offset": 0 }
  }
}
```

### 1.3 Update Project
* **Endpoint**: `PUT /builder/projects/{id}`
* **Body**: Same as Create Project, but all fields are optional.

### 1.4 Delete Project
Soft-deletes a project (status becomes `deleted`).
* **Endpoint**: `DELETE /builder/projects/{id}`

### 1.5 Attach Unit to Project
Links an individual property unit to a macro project.
* **Endpoint**: `POST /builder/projects/{project_id}/units`
* **Body**: 
```json
{
  "property_id": "<uuid>"
}
```

---

## 2. Public Project Browsing (Unauthenticated)

### 2.1 Get Single Project
Fetches the full detail page of a project. Automatically increments its view count.
* **Endpoint**: `GET /projects/{id}`
* **Response**:
```json
{
  "success": true,
  "data": {
    // ... standard project fields
    "builder_info": {
      "id": "...",
      "name": "Skyline Developers",
      "logo": "https://...",
      "is_verified": true
    },
    "review_summary": {
      "average_rating": 4.5,
      "total_reviews": 12
    },
    "related_units": [
      // array of property unit objects attached to this project
    ]
  }
}
```

### 2.2 Search All Projects
* **Endpoint**: `GET /projects`
* **Query Params**:
  * `city` (optional): ILIKE search filter.
  * `project_type` (optional): Exact match string.
  * `status` (optional): Exact match string.
  * `limit` & `offset` (optional).

### 2.3 Enquire on Project (Leads)
Capture a consumer's interest on a project. Drops straight into the builder's CRM dashboard.
* **Endpoint**: `POST /projects/{id}/enquire`
* **Body**:
```json
{
  "name": "Priya S",
  "phone": "9876500000",
  "message": "Interested in 3BHK",
  "preferred_visit_date": "2026-07-10"
}
```

---

## 3. Site Visits (Updated)

### 3.1 Book Visit against a Project
Consumers can now book a visit using either `property_id` OR `project_id`. The booking engine handles both gracefully.
* **Endpoint**: `POST /visits` (Existing route)
* **Headers**: `Authorization: Bearer <token>`
* **Body**:
```json
{
  "project_id": "<uuid>", // New optional field
  "property_id": "<uuid>", // Existing optional field
  "provider_id": "<uuid>", // The ID of the builder
  "scheduled_date_time": "2026-07-10T10:00:00Z",
  "contact_number": "9876543210"
}
```
*Note: Exactly one of `project_id` or `property_id` MUST be provided.*

### 3.2 Builder Visits View
Fetch all visits booked against the builder's properties and projects collectively.
* **Endpoint**: `GET /builder/visits`
* **Headers**: `Authorization: Bearer <token>`
* **Query Params**:
  * `status` (optional)
  * `from_date` (optional)
  * `to_date` (optional)
  * `limit` & `offset`

---

## 4. Reviews (Updated)

Consumers can now leave reviews directly on projects (in addition to properties).

### 4.1 Submit Review
* **Endpoint**: `POST /reviews/property` (Existing route handles both)
* **Headers**: `Authorization: Bearer <token>`
* **Body**:
```json
{
  "visit_id": "<uuid>",
  "project_id": "<uuid>", // New optional field
  "property_id": "<uuid>", // Existing optional field
  "rating": 4.5,
  "comment": "Great project!"
}
```
*Note: Exactly one of `project_id` or `property_id` MUST be provided.*
