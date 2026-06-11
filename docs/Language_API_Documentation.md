# 1️⃣1️⃣ Language API Documentation

## 1. Get Available Languages
Retrieves a list of all available languages supported by the platform.

- **Endpoint:** `GET /api/v1/languages`
- **Requires Authentication:** Yes (Bearer Token)

### Request
No request body or query parameters are required.

### Response

**Success (200 OK)**
```json
{
  "success": true,
  "message": "Languages fetched successfully",
  "data": [
    {
      "code": "en",
      "name": "English"
    },
    {
      "code": "es",
      "name": "Spanish"
    }
  ]
}
```

---

## 2. Set Preferred Language
Updates the authenticated user's preferred language.

- **Endpoint:** `PATCH /api/v1/users/me/language`
- **Requires Authentication:** Yes (Bearer Token)

### Request Body
Updates the preferred language for the user. `language_code` must be a valid code returned by the `GET /api/v1/languages` endpoint.

```json
{
  "language_code": "en"
}
```

### Responses

**Success (200 OK)**
```json
{
  "success": true,
  "message": "Language preference updated successfully",
  "data": {
    "language_code": "en"
  }
}
```

**Client Errors**

- **400 Bad Request** (When `language_code` is empty)
```json
{
  "success": false,
  "error": "language_code cannot be empty"
}
```

- **404 Not Found** (When the provided `language_code` does not exist in the system)
```json
{
  "success": false,
  "error": "Language not found. Use GET /api/v1/languages to see available options."
}
```
