# 🚀 Frontend API Integration - Admin Reports (Tasks 44-48)

Hi Team,

The backend implementation for the **Admin Reports module (Tasks 44-48)** is now complete and ready for you to integrate. Below is the comprehensive API documentation for the endpoints you need to consume.

> **Note:** All endpoints require the requester to be authenticated as an Admin. Please ensure the `Authorization: Bearer <token>` header is passed.

---

## 1. Get All Reports (List)
**Task 44**

Fetches a paginated list of unified reports across the platform.

**Endpoint:** `GET /api/admin/reports`

### Query Parameters
| Field | Type | Description |
|-------|------|-------------|
| `limit` | `integer` | Number of records to return (Default: 10, Max: 100). |
| `offset` | `integer` | Number of records to skip (Default: 0). |
| `status` | `string` | Filter by status: `open`, `resolved`, or `dismissed`. |
| `entityType` | `string` | Filter by entity type: `USER`, `PROPERTY`, `COMMUNITY`, or `POST`. |
| `search` | `string` | Search query matching the report reason or the reporter's name. |

---

## 2. Get Report Detail
**Task 45**

Retrieves full details for a specific report, including an enriched snapshot of the reported entity and the reporting history.

**Endpoint:** `GET /api/admin/reports/:id`

### Snapshot Details
The `entitySnapshot` object structure changes dynamically based on the `entityType`:
- **USER**: `{ "first_name": "...", "last_name": "...", "email": "..." }`
- **PROPERTY**: `{ "title": "...", "owner_id": "...", "owner_name": "..." }`
- **COMMUNITY**: `{ "name": "...", "description": "..." }`
- **POST**: `{ "title": "...", "content": "..." }`

---

## 3. Update Report Status & Admin Notes
**Task 46**

Updates a report's status and allows adding internal admin notes.

**Endpoint:** `PATCH /api/admin/reports/:id`
*(Note: It may be `/api/admin/reports/:id/status` depending on exact routing, please verify or use `PATCH /api/admin/reports/:id`)*

### JSON Payload
```json
{
  "status": "dismissed",
  "adminNotes": "User accidentally reported. No action required."
}
```

---

## 4. Execute Compound Action & Resolve
**Task 48**

Executes a moderation action (such as deleting a property or suspending a user) **AND** automatically resolves the report in a single API call. 

**Endpoint:** `POST /api/admin/reports/:id/action`

### JSON Payload
```json
{
  "action": "delete-property"
}
```

**Allowed Actions:** `suspend-user`, `delete-property`, `delist-property`, `delete-news`, `delete-post`, `dismiss`.

> **⚠️ WARNING:** 
> Executing `suspend-user` on a `PROPERTY` report will suspend the **Owner** of the property, not the reporter. 
> The `dismiss` action closes the report without taking action against the entity.

---

## 5. Force Delete Report
**Task 47**

Permanently hard-deletes a report from the database. This does **not** affect the reported entity itself.

**Endpoint:** `DELETE /api/admin/reports/:id/force`

---

Let me know if you run into any issues during integration!
