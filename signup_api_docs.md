# Signup API Documentation

## 1. Endpoint Overview

- **Method:** `POST`
- **URL Route:** `/api/auth/signup`
- **Authentication:** None required.
- **Description:** Registers a new user account, creates a dual notification OTP (Email & SMS), and returns an authentication JSON Web Token (`token`) along with the serialized user profile details.

---

## 2. Request Payload (Frontend to Backend)

The request payload **MUST** be formatted in `camelCase`.

### Request Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `firstName` | `string` | **Yes** | - | The user's first name. |
| `lastName` | `string` | **Yes** | - | The user's last name. |
| `email` | `string` | No | `null` | The user's email address. Highly recommended for account recovery. |
| `password` | `string` | **Yes** | - | The user's secure account password. |
| `phoneNo` | `string` | **Yes** | - | The user's mobile phone number (with country code preferred). |
| `gender` | `string` | **Yes** | - | The user's gender (e.g., "male", "female", "other"). |
| `userRole` | `string` | No | `"user"` | The role of the user. If omitted, it will automatically default to `"user"`. |

> [!CAUTION]
> **CRITICAL RULE:**
> You **MUST NOT** include or send the `associate_type` field in the signup request payload under any circumstances. The backend dynamically handles this and forces it to `null` to establish the base signup state correctly into the database.

---

## 3. Request Example

Here is a clean JSON example of a valid signup request sent by the frontend:

```json
{
  "firstName": "Arjun",
  "lastName": "Sharma",
  "email": "arjun.sharma@example.com",
  "password": "SecurePassword123!",
  "phoneNo": "+919876543210",
  "gender": "male",
  "userRole": "user"
}
```

---

## 4. Response Payload (Backend to Frontend)

- **HTTP Status:** Upon a successful signup, the API will return an **HTTP 201 Created** status.
- **Payload Format:** The backend response payload strictly uses **`snake_case`**. You must parse it accordingly.
- **Associate Type:** Note that the `associate_type` field in the inner `user` object will universally return as `null` at this early stage in the user's lifecycle.

---

## 5. Success Response Example

```json
{
  "success": true,
  "message": "User created successfully. Verification OTP sent to email and phone.",
  "data": {
    "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjNlNDU2... (JWT Token)",
    "user": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "first_name": "Arjun",
      "last_name": "Sharma",
      "email": "arjun.sharma@example.com",
      "phone_no": "+919876543210",
      "user_role": "user",
      "verified": false,
      "status": "active",
      "associate_type": null,
      "created_at": "2026-03-03T10:00:00.000000Z"
    }
  }
}
```

---

## 6. Error Codes

If the signup request fails, the API will return a JSON error payload with one of the following HTTP status codes mapping to the trigger event:

| HTTP Status | Trigger / Scenario |
|:---:|---|
| **`400 Bad Request`** | Data validation failed (e.g., missing required fields like `firstName` or `password`, malformed JSON structure) or a direct database error prevented user creation. |
| **`409 Conflict`** | Information duplicate constraint. A user account with the provided `email` or `phoneNo` already exists in the system. |
| **`500 Internal Server Error`** | Only triggered if a catastrophic error occurs, such as a failure to create the encrypted JWT token after saving the user to the database. |
