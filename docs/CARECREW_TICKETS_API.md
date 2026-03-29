# CareCrew Support — Ticketing API Docs
> **Base URL:** `http://localhost:9090` | **Date:** 2026-02-23
> All ticketing endpoints require `Authorization: Bearer <token>`

---

## Table of Contents
1. [Overview](#overview)
2. [Data Types & Enums](#data-types--enums)
3. [Create Ticket](#1-create-ticket)
4. [List My Tickets](#2-list-my-tickets)
5. [Get Ticket Detail](#3-get-ticket-detail)
6. [Update Ticket](#4-update-ticket)
7. [Add Comment](#5-add-comment)
8. [Ticket Status Flow](#ticket-status-flow)
9. [Error Codes](#error-codes)

---

## Overview

The CareCrew Ticketing module allows users to raise and track support tickets for property issues, service provider complaints, billing problems, etc.

All endpoints require a valid Bearer token — get it from `POST /api/auth/signin`.

```http
Authorization: Bearer <jwt_token>
```

---

## Data Types & Enums

### `priority` values
| Value | Description |
|-------|-------------|
| `LOW` | Non-urgent issue |
| `MEDIUM` | Default — moderate urgency |
| `HIGH` | Urgent, needs quick resolution |

### `status` values
| Value | Description |
|-------|-------------|
| `OPEN` | Newly created ticket |
| `IN_PROGRESS` | Agent working on it |
| `RESOLVED` | Issue fixed |
| `CLOSED` | Ticket permanently closed |

### `issue_type` values (free-form, suggested values)
`service` | `operational` | `billing` | `property` | `provider_complaint` | `other`

---

## 1. Create Ticket

```http
POST /api/v1/carecrew/tickets
Authorization: Bearer <token>
Content-Type: application/json
```

#### Request Body

```json
{
  "issue_type":  "service",
  "description": "The plumber assigned to my booking did not show up at the scheduled time.",
  "priority":    "HIGH",
  "property_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `issue_type` | string | ✅ | Type of issue — see enum above |
| `description` | string | ✅ | Full description of the problem |
| `priority` | string | ❌ | `LOW` \| `MEDIUM` \| `HIGH` — defaults to `MEDIUM` |
| `property_id` | UUID string | ❌ | Link to a property (optional) |

#### Response `201 Created`

```json
{
  "success": true,
  "message": "Ticket created successfully",
  "data": {
    "ticket": {
      "id":          "uuid",
      "userId":      "uuid",
      "propertyId":  "550e8400-e29b-41d4-a716-446655440000",
      "assigneeId":  null,
      "issueType":   "service",
      "description": "The plumber assigned to my booking did not show up at the scheduled time.",
      "priority":    "HIGH",
      "status":      "OPEN",
      "createdAt":   "2026-02-23T18:15:00Z",
      "updatedAt":   "2026-02-23T18:15:00Z"
    }
  }
}
```

#### Errors
| Code | HTTP | When |
|------|------|------|
| `UNAUTHORIZED` | 401 | No / invalid token |
| `MISSING_FIELDS` | 400 | `issue_type` or `description` missing |
| `INVALID_PRIORITY` | 400 | Priority not `LOW/MEDIUM/HIGH` |
| `DB_ERROR` | 500 | Database error |

---

## 2. List My Tickets

```http
GET /api/v1/carecrew/tickets
Authorization: Bearer <token>
```

Returns paginated list of tickets for the authenticated user.

#### Query Params

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `userId` | UUID string | ❌ (uses token) | Filter by user ID (admin use) |
| `status` | string | — | Filter by status |
| `priority` | string | — | Filter by priority |
| `page` | integer | `1` | Page number |
| `limit` | integer | `10` | Max 50 per page |

#### Example

```http
GET /api/v1/carecrew/tickets?status=OPEN&page=1&limit=10
```

#### Response `200 OK`

```json
{
  "success": true,
  "message": "Tickets retrieved successfully",
  "data": {
    "tickets": [
      {
        "id":          "uuid",
        "userId":      "uuid",
        "propertyId":  null,
        "assigneeId":  null,
        "issueType":   "billing",
        "description": "I was charged twice for the same booking.",
        "priority":    "HIGH",
        "status":      "OPEN",
        "createdAt":   "2026-02-20T10:00:00Z",
        "updatedAt":   "2026-02-20T10:00:00Z"
      }
    ],
    "pagination": {
      "total_count":  5,
      "current_page": 1,
      "total_pages":  1,
      "limit":        10
    }
  }
}
```

---

## 3. Get Ticket Detail

```http
GET /api/v1/carecrew/tickets/{ticketId}
Authorization: Bearer <token>
```

Returns full ticket details including all comments.

#### Path Params

| Param | Type | Description |
|-------|------|-------------|
| `ticketId` | UUID | Ticket ID |

#### Response `200 OK`

```json
{
  "success": true,
  "message": "Ticket retrieved successfully",
  "data": {
    "ticket": {
      "id":          "uuid",
      "userId":      "uuid",
      "propertyId":  null,
      "assigneeId":  "agent-uuid",
      "issueType":   "service",
      "description": "Provider did not show up.",
      "priority":    "HIGH",
      "status":      "IN_PROGRESS",
      "createdAt":   "2026-02-23T18:15:00Z",
      "updatedAt":   "2026-02-23T19:00:00Z"
    },
    "comments": [
      {
        "id":          "uuid",
        "ticketId":    "uuid",
        "commenterId": "agent-uuid",
        "comment":     "We have contacted the provider and are investigating.",
        "createdAt":   "2026-02-23T18:45:00Z"
      }
    ]
  }
}
```

#### Errors
| Code | HTTP | When |
|------|------|------|
| `INVALID_UUID` | 400 | `ticketId` is not a valid UUID |
| `NOT_FOUND` | 404 | Ticket does not exist |
| `FORBIDDEN` | 403 | Ticket belongs to another user |

---

## 4. Update Ticket

```http
PATCH /api/v1/carecrew/tickets/{ticketId}
Authorization: Bearer <token>
Content-Type: application/json
```

Update ticket **status** and/or **assign** an agent. Both fields are optional — send only what needs updating.

#### Request Body

```json
{
  "status":      "IN_PROGRESS",
  "assignee_id": "agent-uuid"
}
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `status` | string | ❌ | Must follow valid state transitions (see below) |
| `assignee_id` | UUID string | ❌ | Agent UUID to assign |

#### Response `200 OK`

```json
{
  "success": true,
  "message": "Ticket updated successfully",
  "data": {
    "ticket": {
      "id":         "uuid",
      "status":     "IN_PROGRESS",
      "assigneeId": "agent-uuid",
      "updatedAt":  "2026-02-23T19:00:00Z"
    }
  }
}
```

#### Errors
| Code | HTTP | When |
|------|------|------|
| `INVALID_STATUS` | 400 | Status not in `OPEN/IN_PROGRESS/RESOLVED/CLOSED` |
| `INVALID_TRANSITION` | 422 | e.g. `OPEN → CLOSED` (must go step by step) |
| `TICKET_CLOSED` | 422 | Ticket is already CLOSED — no more changes allowed |
| `NOT_FOUND` | 404 | Ticket not found |
| `INVALID_UUID` | 400 | Bad UUID format |

---

## 5. Add Comment

```http
POST /api/v1/carecrew/tickets/{ticketId}/comments
Authorization: Bearer <token>
Content-Type: application/json
```

Add a comment to a ticket (visible to user and support agents).

#### Request Body

```json
{
  "comment": "I can confirm the issue happened at 10AM. Please expedite."
}
```

| Field | Type | Required |
|-------|------|----------|
| `comment` | string | ✅ |

#### Response `201 Created`

```json
{
  "success": true,
  "message": "Comment added successfully",
  "data": {
    "comment": {
      "id":          "uuid",
      "ticketId":    "uuid",
      "commenterId": "user-uuid",
      "comment":     "I can confirm the issue happened at 10AM. Please expedite.",
      "createdAt":   "2026-02-23T19:30:00Z"
    }
  }
}
```

#### Errors
| Code | HTTP | When |
|------|------|------|
| `MISSING_FIELDS` | 400 | `comment` is empty |
| `NOT_FOUND` | 404 | Ticket not found |
| `TICKET_CLOSED` | 422 | Cannot comment on a CLOSED ticket |

---

## Ticket Status Flow

```
   ┌──────────────────────────────────────────────────┐
   │                                                  │
   ▼                                                  │
 OPEN  ──────────►  IN_PROGRESS  ──────────►  RESOLVED  ──────────►  CLOSED
                        │                       │
                        │ (re-open)              │ (re-open)
                        ▼                       ▼
                       OPEN                 IN_PROGRESS
```

| From | To | Allowed? |
|------|----|----------|
| `OPEN` | `IN_PROGRESS` | ✅ |
| `IN_PROGRESS` | `RESOLVED` | ✅ |
| `IN_PROGRESS` | `OPEN` *(re-open)* | ✅ |
| `RESOLVED` | `CLOSED` | ✅ |
| `RESOLVED` | `IN_PROGRESS` *(re-open)* | ✅ |
| `OPEN` | `RESOLVED` | ❌ (skip not allowed) |
| `OPEN` | `CLOSED` | ❌ (skip not allowed) |
| `CLOSED` | anything | ❌ (terminal — forever) |

> ⚠️ `CLOSED` is **permanent** — once closed, a ticket cannot be re-opened or modified.

---

## All Ticketing Endpoints

| Method | Endpoint | Auth | Description |
|--------|----------|------|-------------|
| POST | `/api/v1/carecrew/tickets` | 🔒 | Create a new ticket |
| GET | `/api/v1/carecrew/tickets` | 🔒 | List my tickets (paginated) |
| GET | `/api/v1/carecrew/tickets/{ticketId}` | 🔒 | Ticket detail + comments |
| PATCH | `/api/v1/carecrew/tickets/{ticketId}` | 🔒 | Update status / assign agent |
| POST | `/api/v1/carecrew/tickets/{ticketId}/comments` | 🔒 | Add a comment |

---

## Error Codes

| Code | HTTP | Meaning |
|------|------|---------|
| `UNAUTHORIZED` | 401 | Missing or invalid Bearer token |
| `FORBIDDEN` | 403 | Resource belongs to another user |
| `INVALID_UUID` | 400 | UUID in path or body is malformed |
| `MISSING_FIELDS` | 400 | Required field missing in body |
| `INVALID_PRIORITY` | 400 | Priority not in `LOW/MEDIUM/HIGH` |
| `INVALID_STATUS` | 400 | Status not in `OPEN/IN_PROGRESS/RESOLVED/CLOSED` |
| `INVALID_TRANSITION` | 422 | Status machine violation |
| `TICKET_CLOSED` | 422 | Ticket is terminal — no more updates |
| `NOT_FOUND` | 404 | Ticket not found |
| `DB_ERROR` | 500 | Database error |
