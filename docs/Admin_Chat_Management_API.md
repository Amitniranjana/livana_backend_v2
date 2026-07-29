# Admin Chat Management APIs

This document outlines the newly added Admin APIs to manage and monitor user-to-user chats. These APIs are protected by the `admin_auth_guard` middleware. All requests require a valid admin session cookie (`admin_session`) or an `Authorization: Bearer <admin_token>` header.

## 1. Get All User Chats
Retrieves a paginated list of user-to-user chats, including participant details and block/archive statuses.

**Endpoint**: `GET /api/admin/chats`
**Auth Required**: Yes (Admin)

### Query Parameters
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `page` | `integer` | No | Page number (default: 1) |
| `limit` | `integer` | No | Number of chats per page (default: 50) |
| `search` | `string` | No | Search term to filter by participant's `name` or `email` |
| `isBlocked` | `boolean` | No | Filter chats where participants have blocked each other |
| `isArchived` | `boolean` | No | Filter chats that have been archived by any participant |

### Response Example
```json
{
  "success": true,
  "data": {
    "total": 120,
    "chats": [
      {
        "id": "123e4567-e89b-12d3-a456-426614174000",
        "name": null,
        "createdAt": "2026-07-27T10:00:00Z",
        "participants": [
          {
            "id": "123e4567-e89b-12d3-a456-426614174001",
            "name": "John Doe",
            "email": "john@example.com",
            "profilePicture": "https://url.to.pic",
            "joinedAt": "2026-07-27T10:00:00Z"
          }
        ],
        "lastMessage": "Hello there!",
        "lastMessageAt": "2026-07-27T10:05:00Z",
        "isBlocked": false,
        "isArchived": false
      }
    ]
  }
}
```

---

## 2. Get Chat Messages
Retrieves all messages for a specific chat.

**Endpoint**: `GET /api/admin/chats/:id/messages`
**Auth Required**: Yes (Admin)

### Query Parameters
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `page` | `integer` | No | Page number (default: 1) |
| `limit` | `integer` | No | Number of messages per page (default: 50) |

### Response Example
```json
{
  "success": true,
  "data": {
    "total": 45,
    "messages": [
      {
        "id": "123e4567-e89b-12d3-a456-426614174003",
        "chatId": "123e4567-e89b-12d3-a456-426614174000",
        "senderId": "123e4567-e89b-12d3-a456-426614174001",
        "senderName": "John Doe",
        "senderEmail": "john@example.com",
        "content": "Hello there!",
        "createdAt": "2026-07-27T10:05:00Z"
      }
    ]
  }
}
```

---

## 3. Force Delete Chat
Permanently deletes a chat. This action will cascade and delete all related messages and participant records. It creates an entry in `admin_action_logs`.

**Endpoint**: `DELETE /api/admin/chats/:id/force`
**Auth Required**: Yes (Admin)

### Response Example
```json
{
  "success": true,
  "message": "Chat deleted successfully"
}
```
*(Returns 404 if chat is not found)*

---

## 4. Force Delete Message
Permanently deletes a single message without removing the entire chat. It creates an entry in `admin_action_logs`.

**Endpoint**: `DELETE /api/admin/messages/:id/force`
**Auth Required**: Yes (Admin)

### Response Example
```jsonAA^C
ubuntu@ip-172-31-23-134:~$ nc -zv database-1.c8n64wqukf20.us-east-1.rds.amazonaws.com 5432
Connection to database-1.c8n64wqukf20.us-east-1.rds.amazonaws.com (172.31.82.87) 5432 port [tcp/postgresql] succeeded!
ubuntu@ip-172-31-23-134:~$ ^C
ubuntu@ip-172-31-23-134:~$ 
{
  "success": true,
  "message": "Message deleted successfully"
}
```
*(Returns 404 if message is not found)*
