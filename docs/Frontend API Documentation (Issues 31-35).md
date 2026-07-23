# Frontend API Documentation (Issues 31-35)

This document outlines the API endpoints, required authentication, request payloads, and response structures for the newly implemented CareCrew, Admin Chat, and Admin Reports features.

---

## 1. Single CareCrew Member Profile
**Endpoint:** `GET /api/carecrew/{id}`

- **Description:** Fetches the public profile details for a single CareCrew member (provider).
- **Authentication:** Public (No Auth required).
- **Path Parameters:**
  - `id` (UUID): The unique identifier of the CareCrew provider.

**Response Example:**
```json
{
  "success": true,
  "message": "Provider retrieved successfully",
  "data": {
    "provider": {
      "id": "uuid",
      "name": "John Doe",
      "photo": "https://url...",
      "city": "Mumbai",
      "services_offered": [...],
      "rating": 4.5,
      "verified_kyc_status": "approved",
      // phone and email are only included if the request includes valid User Auth
      "phone": "+919876543210",
      "email": "john@example.com"
    }
  }
}
```

---

## 2. Admin Chat: Send a Message
**Endpoint:** `POST /api/chat/admin/messages`

- **Description:** Send a message in a user-admin chat thread. If sent by a user without an existing thread, a thread will be automatically created.
- **Authentication:** Required (Valid User JWT **or** Admin Cookie Session).
- **Headers:** 
  - User: `Authorization: Bearer <token>`
  - Admin: Cookie `admin_session=<token>`

**Request Body:**
```json
{
  "thread_id": "uuid", // Optional for users. Required for admins.
  "message": "Hello, I need help with my account.",
  "attachment_url": "https://url-to-attachment..." // Optional
}
```

**Response Example:**
```json
{
  "success": true,
  "message": "Message sent successfully"
}
```

---

## 3. Admin Chat: Get Thread Messages
**Endpoint:** `GET /api/chat/admin/threads/{thread_id}/messages`

- **Description:** Fetch paginated message history for a specific support thread.
- **Authentication:** Required (User on the thread **or** Admin).
- **Path Parameters:**
  - `thread_id` (UUID): The ID of the thread.
- **Query Parameters:**
  - `limit` (int, default: 50): Number of messages to return.
  - `offset` (int, default: 0): Pagination offset.
  - `since` (ISO 8601 DateTime, optional): Fetch messages created after this timestamp (useful for polling).

**Response Example:**
```json
{
  "success": true,
  "data": {
    "total": 1,
    "messages": [
      {
        "id": "uuid",
        "thread_id": "uuid",
        "sender_id": "uuid",
        "sender_role": "user", // "user" or "admin"
        "message": "Hello, I need help with my account.",
        "attachment_url": null,
        "created_at": "2026-07-22T10:00:00Z"
      }
    ]
  }
}
```

---

## 4. Admin Chat: List Threads
**Endpoint:** `GET /api/chat/admin/threads`

- **Description:** 
  - For Users: Returns their own support thread (or empty list if none exists).
  - For Admins: Returns all support threads, acting as an inbox.
- **Authentication:** Required (User JWT **or** Admin Cookie).
- **Query Parameters (Admin Only):**
  - `status` (string, optional): Filter by thread status (e.g., "open", "closed").
  - `user_id` (UUID, optional): Filter threads by a specific user ID.

**Response Example:**
```json
{
  "success": true,
  "data": [
    {
      "id": "uuid",
      "user_id": "uuid",
      "admin_id": "uuid", // Can be null if no admin has replied yet
      "status": "open",
      "created_at": "2026-07-21T10:00:00Z",
      "updated_at": "2026-07-22T10:00:00Z",
      "last_message": "Hello, I need help with my account."
    }
  ]
}
```

---

## 5. Admin Reports: List Property Reports
**Endpoint:** `GET /api/admin/reports`

- **Description:** List all reports made by users on properties for admin review.
- **Authentication:** Admin Required (Admin Cookie Session).
- **Query Parameters:**
  - `status` (string, optional): e.g., "open", "reviewed", "dismissed", "action_taken"
  - `property_id` (UUID, optional): Filter reports for a specific property.
  - `limit` (int, default: 10): Number of records per page.
  - `offset` (int, default: 0): Pagination offset.

**Response Example:**
```json
{
  "success": true,
  "data": {
    "reports": [
      {
        "id": "uuid",
        "reporter_user": {
          "id": "uuid",
          "name": "Jane Doe",
          "email": "jane@example.com"
        },
        "property_id": "uuid",
        "property_snapshot": {
          "title": "Luxury Villa in Andheri",
          "owner_id": "uuid",
          "owner_name": "Broker Bob"
        },
        "reason": "inaccurate_info",
        "comment": "The pricing is incorrect.",
        "status": "open",
        "created_at": "2026-07-22T10:00:00Z"
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

## 6. Admin Reports: Single Report Details
**Endpoint:** `GET /api/admin/reports/{id}`

- **Description:** Full detail of a single report, including the report history for that specific property.
- **Authentication:** Admin Required (Admin Cookie Session).
- **Path Parameters:**
  - `id` (UUID): The ID of the report.

**Response Example:**
```json
{
  "success": true,
  "data": {
    "report": {
      // Same structure as a single item in the list response
      "id": "uuid",
      "reporter_user": { ... },
      "property_id": "uuid",
      "property_snapshot": { ... },
      "reason": "inaccurate_info",
      "comment": "The pricing is incorrect.",
      "status": "open",
      "created_at": "2026-07-22T10:00:00Z"
    },
    "report_history": [
      {
        "id": "uuid", // ID of another report on the same property
        "reason": "spam",
        "status": "dismissed",
        "created_at": "2026-07-10T10:00:00Z"
      }
    ]
  }
}
```
