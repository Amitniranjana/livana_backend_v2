# Chat API Documentation

This document outlines the endpoints related to the Chat functionality in the Livana Care Connect App.

---

## `GET /api/chats` 🔒

Retrieves all chats (both recent and old) for the authenticated user. This includes the unread message count and the last message of each chat thread to populate a chat list screen efficiently.

**Auth Required:** `Yes` (Bearer Token)

### Request

```http
GET /api/chats
Authorization: Bearer <jwt_token>
```

#### Headers
| Header | Value | Required | Description |
|--------|-------|----------|-------------|
| `Authorization` | `Bearer <token>` | Yes | Valid JWT access token |

### Success Response `200 OK`

Returns a list of chat items, sorted with the most recently active chats appearing first.

```json
{
  "success": true,
  "data": [
    {
      "chat_id": "550e8400-e29b-41d4-a716-446655440000",
      "participant": {
        "id": "550e8400-e29b-41d4-a716-446655440001",
        "name": "Rahul Sharma",
        "profile_image": "https://cdn.example.com/photos/rahul.jpg"
      },
      "last_message": {
        "text": "Hello, is this property available?",
        "timestamp": "2026-03-24T12:00:00Z"
      },
      "unread_count": 0
    },
    {
      "chat_id": "550e8400-e29b-41d4-a716-446655440002",
      "participant": {
        "id": "550e8400-e29b-41d4-a716-446655440003",
        "name": "Priya Singh",
        "profile_image": null
      },
      "last_message": {
        "text": "Theek hai, kal milte hain",
        "timestamp": "2026-03-20T08:30:00Z"
      },
      "unread_count": 0
    }
  ]
}
```

#### Notes on the Response Data:
- `data.participant.name` string concatenates the actual `first_name` and `last_name` from the database.
- `data.last_message` relies on the `content` field of the `messages` table and will be `null` if the chat has no messages.
- `data.unread_count` currently returns `0` statically by default, since read-receipts tracking (`is_read` column) is not active in the current database schema implementation.

### Error Responses

#### `401 Unauthorized` (Invalid/Missing Token)
```json
{
  "success": false,
  "message": "Unauthorized. Invalid user ID format in token.",
  "error_code": "INVALID_TOKEN"
}
```

#### `500 Internal Server Error` (Database Issue)
```json
{
  "success": false,
  "message": "Internal server error",
  "error_code": "DATABASE_ERROR"
}
```
