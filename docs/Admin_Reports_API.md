# Unified Admin Reports API Documentation

This document describes the unified endpoints for managing reports across the platform (Users, Properties, Communities, and Posts/News) for the Admin Dashboard.

> [!NOTE]
> All endpoints require the requester to be authenticated as an Admin. Ensure the `Authorization: Bearer <token>` header is passed with an active admin JWT.

---

## 1. Get All Reports (List)

Retrieve a paginated list of all reports across the platform with optional filters.

**Endpoint:** `GET /api/admin/reports`

### Query Parameters

| Field | Type | Description |
|-------|------|-------------|
| `limit` | `integer` | Number of records to return (Default: 10, Max: 100). |
| `offset` | `integer` | Number of records to skip (Default: 0). |
| `status` | `string` | Filter by status: `open`, `resolved`, or `dismissed`. |
| `entityType` | `string` | Filter by entity type: `USER`, `PROPERTY`, `COMMUNITY`, or `POST`. |
| `search` | `string` | Search query matching the report reason or the reporter's name. |

### Response Example

```json
{
  "success": true,
  "data": {
    "reports": [
      {
        "id": "e44d32a0-41ab-40a1-94ff-b4e8c1b99b50",
        "reporter_user": {
          "id": "5fa23d13-64af-4b82-84bc-872abfe481c0",
          "name": "John Doe",
          "email": "john@example.com"
        },
        "entityType": "PROPERTY",
        "entityId": "203a95c9-9404-4530-9b4f-8012b1d3cf12",
        "entitySnapshot": null,
        "reason": "Inappropriate content",
        "comment": "Spam",
        "adminNotes": null,
        "status": "open",
        "created_at": "2026-08-16T12:00:00Z"
      }
    ],
    "pagination": {
      "total": 1,
      "limit": 10,
      "offset": 0
    }
  }
}
```

---

## 2. Get Report Detail

Retrieve full details for a specific report, including an enriched snapshot of the reported entity and the reporting history for that entity.

**Endpoint:** `GET /api/admin/reports/:id`

### Response Example

```json
{
  "success": true,
  "data": {
    "report": {
      "id": "e44d32a0-41ab-40a1-94ff-b4e8c1b99b50",
      "reporter_user": {
        "id": "5fa23d13-64af-4b82-84bc-872abfe481c0",
        "name": "John Doe",
        "email": "john@example.com"
      },
      "entityType": "PROPERTY",
      "entityId": "203a95c9-9404-4530-9b4f-8012b1d3cf12",
      "entitySnapshot": {
        "title": "Beautiful Villa",
        "owner_id": "a4fa95c9-2342-4530-9b4f-8012b1d3c111",
        "owner_name": "Jane Smith"
      },
      "reason": "Inappropriate content",
      "comment": "Spam",
      "adminNotes": "Checking this right now",
      "status": "open",
      "created_at": "2026-08-16T12:00:00Z"
    },
    "report_history": [
      {
        "id": "8bbd32a0-12ab-40a1-94ff-b4e8c1b99c11",
        "reason": "Fake listing",
        "status": "dismissed",
        "created_at": "2026-08-10T14:30:00Z"
      }
    ]
  }
}
```

> [!NOTE]
> The `entitySnapshot` object structure changes dynamically based on the `entityType` (`PROPERTY`, `USER`, `COMMUNITY`, `POST`). 
> - **USER**: `{ "first_name": "...", "last_name": "...", "email": "..." }`
> - **PROPERTY**: `{ "title": "...", "owner_id": "...", "owner_name": "..." }`
> - **COMMUNITY**: `{ "name": "...", "description": "..." }`
> - **POST**: `{ "title": "...", "content": "..." }`

---

## 3. Update Report Status & Admin Notes

Manually update a report's status and add internal admin notes.

**Endpoint:** `PATCH /api/admin/reports/:id/status`

### JSON Body Payload

| Field | Type | Description |
|-------|------|-------------|
| `status` | `string` | The new status (e.g., `open`, `resolved`, `dismissed`). |
| `adminNotes` | `string` | Optional. Internal notes logged by the admin. |

### Example Request

```json
{
  "status": "dismissed",
  "adminNotes": "User accidentally reported. No action required."
}
```

### Example Response

```json
{
  "success": true,
  "message": "Report updated"
}
```

---

## 4. Execute Compound Action & Resolve

Executes a moderation action (such as deleting a property or suspending a user) AND automatically resolves the report in a single API call. This is highly recommended for quick moderation and automatically logs the action in the Admin Audit logs.

**Endpoint:** `POST /api/admin/reports/:id/action`

### JSON Body Payload

| Field | Type | Description |
|-------|------|-------------|
| `action` | `string` | The moderation action to execute. Allowed values: `suspend-user`, `delete-property`, `delist-property`, `delete-news`, `delete-post`, `dismiss`. |

> [!WARNING]
> Executing `suspend-user` on a `PROPERTY` report will suspend the **Owner** of the property, not the reporter. Similarly, executing `delete-property` on a property report will instantly remove the property from the platform. 
> The `dismiss` action simply closes the report without taking any action against the entity.

### Example Request

```json
{
  "action": "delete-property"
}
```

### Example Response

```json
{
  "success": true,
  "message": "Action executed and report resolved"
}
```

---

## 5. Force Delete Report

Permanently hard-deletes a report from the database. This does NOT affect the reported entity itself.

**Endpoint:** `DELETE /api/admin/reports/:id/force`

### Example Response

```json
{
  "success": true,
  "message": "Report deleted successfully"
}
```
