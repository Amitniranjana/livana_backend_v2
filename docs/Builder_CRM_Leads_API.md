# Builder CRM Leads API Documentation

**Base URL**: `/api/builder/crm-leads`  
**Authentication**: Bearer JWT (Requires `builder` role)

This module allows builders to manage external CRM leads (from 99acres, MagicBricks, etc.) within the Livana platform.

## Lead Object Structure

```json
{
  "id": "uuid",
  "user_id": "uuid",
  "project_id": "uuid|null",
  "project_name": "string|null",
  "source": "99acres",
  "source_detail": "string|null",
  "name": "string",
  "phone": "string",
  "email": "string|null",
  "budget_min": 0,
  "budget_max": 0,
  "requirement": "string|null",
  "location_preference": "string|null",
  "status": "new",
  "priority": "warm",
  "notes": "string|null",
  "next_follow_up_date": "2026-08-10|null",
  "created_at": "2026-08-01T00:00:00Z",
  "updated_at": "2026-08-01T00:00:00Z"
}
```

### Enumerations
All enumeration values are strictly validated server-side.

- **source**: `99acres` | `magicbricks` | `housing` | `nobroker` | `facebook_ads` | `google_ads` | `referral` | `walk_in` | `other`
- **status**: `new` | `contacted` | `site_visit_scheduled` | `negotiation` | `converted` | `lost`
- **priority**: `hot` | `warm` | `cold`

---

## Endpoints

### 1. Get CRM Leads

Retrieve a paginated list of CRM leads for the authenticated builder.

**Endpoint**: `GET /api/builder/crm-leads`

**Query Parameters (Optional)**:
- `status` (string) - Filter by lead status
- `source` (string) - Filter by lead source
- `priority` (string) - Filter by lead priority
- `search` (string) - Case-insensitive search across name, phone, or email (ILIKE)
- `limit` (integer) - Results per page (default: 50, max: 1000)
- `offset` (integer) - Pagination offset (default: 0)

**Response**:
```json
{
  "success": true,
  "message": "",
  "data": {
    "leads": [ /* Array of Lead Objects */ ],
    "pagination": {
      "total": 12,
      "limit": 50,
      "offset": 0
    }
  }
}
```
*Note for Export: To export data on the client side, call this endpoint with a high limit (e.g. 1000) and currently-applied filters to retrieve the raw JSON data.*

---

### 2. Create CRM Lead

Create a new CRM lead. 

**Endpoint**: `POST /api/builder/crm-leads`

**Request Body**:
```json
{
  "project_id": "uuid", // Optional
  "source": "99acres", // Required (must be valid enum)
  "source_detail": "Details...", // Optional
  "name": "John Doe", // Required
  "phone": "+919876543210", // Required
  "email": "john@example.com", // Optional
  "budget_min": 5000000, // Optional
  "budget_max": 10000000, // Optional
  "requirement": "3BHK, high floor", // Optional
  "location_preference": "South Delhi", // Optional
  "status": "new", // Optional (must be valid enum, defaults to "new")
  "priority": "warm", // Optional (must be valid enum, defaults to "warm")
  "notes": "Follow up next week", // Optional
  "next_follow_up_date": "2026-08-10" // Optional (YYYY-MM-DD)
}
```

**Response**:
```json
{
  "success": true,
  "message": "CRM Lead created",
  "data": { /* Created Lead Object */ }
}
```

---

### 3. Update CRM Lead

Update an existing CRM lead.

**Endpoint**: `PUT /api/builder/crm-leads/:id`

**Request Body**: *(Same structure as Create CRM Lead payload)*

**Response**:
```json
{
  "success": true,
  "message": "CRM Lead updated",
  "data": { /* Updated Lead Object */ }
}
```

---

### 4. Update Lead Status (Quick Action)

Update only the status of a specific lead.

**Endpoint**: `PATCH /api/builder/crm-leads/:id/status`

**Request Body**:
```json
{
  "status": "contacted" // Required (must be valid enum)
}
```

**Response**:
```json
{
  "success": true,
  "message": "Status updated",
  "data": {
    "id": "uuid",
    "status": "contacted"
  }
}
```

---

### 5. Delete CRM Lead

Delete a specific lead.

**Endpoint**: `DELETE /api/builder/crm-leads/:id`

**Response**:
```json
{
  "success": true,
  "message": "Lead deleted",
  "data": null
}
```
