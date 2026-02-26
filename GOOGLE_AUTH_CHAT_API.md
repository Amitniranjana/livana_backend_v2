# Livana API Documentation — Auth & Chat Endpoints

**Base URL:** `http://<your-server>:9090`
**Content-Type:** `application/json`

---

## 1. Google Sign-In

### `POST /auth/google`

Sign in with a Google account. Send the `id_token` received from the **Google Identity Services (GIS)** SDK.

**Request Body**
```json
{
  "id_token": "eyJhbGciOiJSUzI1NiIsImtpZCI6..."
}
```

**Success Response — `200 OK`**
```json
{
  "success": true,
  "message": "Signed in with Google successfully",
  "data": {
    "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "user": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "email": "alice@gmail.com",
      "name": "Alice Smith",
      "picture": "https://lh3.googleusercontent.com/a/photo"
    }
  }
}
```

**Error Responses**

| Status | Reason |
|--------|--------|
| `401` | Invalid / expired Google token |
| `422` | Missing `id_token` field in body |
| `500` | Server / DB error |

**Frontend Notes**
- Use the `access_token` from the response as a **Bearer token** for all protected API calls.
- Store it in memory or an HTTP-only cookie. Avoid `localStorage` in production.
- Token expires in **1 hour**. Re-call this endpoint with a fresh Google token to get a new one.
- New users are **auto-created** on first sign-in — no separate registration needed.

**How to get `id_token` (Google GIS SDK)**
```html
<script src="https://accounts.google.com/gsi/client" async defer></script>
<script>
  google.accounts.id.initialize({
    client_id: 'YOUR_GOOGLE_CLIENT_ID.apps.googleusercontent.com',
    callback: (response) => {
      const idToken = response.credential; // send this to POST /auth/google
    }
  });
  google.accounts.id.renderButton(document.getElementById('btn'), { theme: 'outline' });
</script>
```

---

## 2. Recent Chats

### `GET /api/v1/chats/recent`

Fetch the authenticated user's recent chat conversations, sorted by the **latest message time** (newest first).

> 🔒 **Requires authentication.** Send the `access_token` from the sign-in response.

**Request Headers**
```
Authorization: Bearer <access_token>
```

**Success Response — `200 OK`**
```json
{
  "success": true,
  "message": "Recent chats fetched successfully",
  "data": [
    {
      "chat_id": "3fa85f64-5717-4562-b3fc-2c963f66afa6",
      "last_message": "Hey, are you available?",
      "last_message_time": "2026-02-26T17:30:00Z"
    },
    {
      "chat_id": "7c9e6679-7425-40de-944b-e07fc1f90ae7",
      "last_message": "Property looks great!",
      "last_message_time": "2026-02-26T14:00:00Z"
    }
  ]
}
```

> Returns an **empty array `[]`** if the user has no chats yet — not an error.

**Error Responses**

| Status | Reason |
|--------|--------|
| `401` | Missing, expired, or invalid `Authorization` header |
| `500` | Server / DB error |

**Frontend Notes**
- `last_message_time` is **UTC ISO 8601** — convert to local time for display.
- Sort is handled by the server — no need to sort on the frontend.
- `chat_id` is a UUID — use it to navigate to the individual chat screen.

**Example (JavaScript `fetch`)**
```js
const response = await fetch('http://<server>:9090/api/v1/chats/recent', {
  headers: {
    'Authorization': `Bearer ${accessToken}`
  }
});
const { success, data } = await response.json();

if (response.status === 401) {
  // Token expired — trigger re-login
}
```

---

## Quick Reference

| Method | Endpoint | Auth | Description |
|--------|----------|------|-------------|
| `POST` | `/auth/google` | ❌ Public | Google Sign-In, returns JWT |
| `GET` | `/api/v1/chats/recent` | ✅ Bearer JWT | Fetch recent chats |

---

## Standard Response Envelope

All responses follow this shape:
```json
{
  "success": true | false,
  "message": "Human-readable message",
  "data": { ... } | [ ... ] | null
}
```
