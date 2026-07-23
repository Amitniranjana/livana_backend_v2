# Admin Action Logs API Documentation

This document outlines the API endpoints for retrieving admin action logs (audit trails).

## 1. Get History of Actions by Target

Retrieve the history of all admin actions taken on a specific entity (e.g., all actions on one KYC record, one property, or one report). This is specifically useful for an "Activity" tab on a detail page.

- **Endpoint:** `GET /api/admin/logs/{target_type}/{target_id}`
- **Method:** `GET`
- **Auth Required:** Yes (Role: `admin`)

### Path Parameters

| Parameter   | Type   | Description |
| :---        | :---   | :---        |
| `target_type` | `string` | The type of the target entity (e.g., `kyc`, `property`, `report`, `user`). |
| `target_id`   | `uuid`   | The unique identifier of the target entity. |

### Response (Success)

- **Status Code:** `200 OK`
- **Content-Type:** `application/json`

```json
{
  "success": true,
  "data": {
    "total": 2,
    "logs": [
      {
        "id": "e456d817-f273-4e4b-9e32-23c31dbb80b7",
        "admin_id": "admin_uuid",
        "admin_name": null,
        "action_type": "report_resolved",
        "target_type": "report",
        "target_id": "7891d817-f273-4e4b-9e32-23c31dbb8000",
        "details": {
          "new_status": "action_taken",
          "resolution_note": "Listing removed"
        },
        "created_at": "2026-07-23T10:00:00Z"
      }
    ]
  }
}
```

## 2. Get All Admin Logs (Paginated)

Retrieve a paginated list of all admin actions for a global audit trail.

- **Endpoint:** `GET /api/admin/logs`
- **Method:** `GET`
- **Auth Required:** Yes (Role: `admin`)

### Query Parameters

| Parameter     | Type      | Description |
| :---          | :---      | :---        |
| `admin_id`    | `uuid`    | Optional. Filter logs by a specific admin's ID. |
| `action_type` | `string`  | Optional. Filter logs by a specific action (e.g., `kyc_approve`). |
| `target_type` | `string`  | Optional. Filter logs by a specific target type (e.g., `property`). |
| `from_date`   | `datetime`| Optional. Start date (inclusive). |
| `to_date`     | `datetime`| Optional. End date (inclusive). |
| `limit`       | `integer` | Optional. Pagination limit. Default `20`, Max `100`. |
| `offset`      | `integer` | Optional. Pagination offset. Default `0`. |

### Response (Success)

- **Status Code:** `200 OK`
- **Content-Type:** `application/json`

```json
{
  "success": true,
  "data": {
    "total": 100,
    "logs": [
      {
        "id": "a456d817-f273-4e4b-9e32-23c31dbb80b1",
        "admin_id": "admin_uuid",
        "admin_name": null,
        "action_type": "property_approve",
        "target_type": "property",
        "target_id": "9991d817-f273-4e4b-9e32-23c31dbb8001",
        "details": null,
        "created_at": "2026-07-22T08:30:00Z"
      }
    ]
  }
}
```
