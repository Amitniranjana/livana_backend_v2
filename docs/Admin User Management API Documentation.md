# Admin User Management API Documentation

These APIs are protected by a session cookie `admin_session`. The middleware verifies the JWT token present in the cookie and rejects unauthorized requests with a `401 Unauthorized` status.

All endpoints are prefixed with: `/api/admin/users`

---

## 1. List Users

**Endpoint:** `GET /api/admin/users`

**Description:** Retrieves a paginated list of users with advanced filtering and sorting.

**Query Parameters:**
| Parameter      | Type   | Description                                                                                       |
| -------------- | ------ | ------------------------------------------------------------------------------------------------- |
| `search`       | string | ILIKE search on `email`, `first_name`, `last_name`, and `phone_no`.                               |
| `role`         | string | Filter by user role (e.g., `user`, `admin`, `broker`).                                            |
| `associateType`| string | Filter by associate type (e.g., `agency`, `independent`).                                         |
| `status`       | string | Filter by status (`active`, `suspended`, `banned`).                                               |
| `isVerified`   | bool   | Filter by phone/email verification status (`true`, `false`).                                      |
| `kycStatus`    | string | Filter by the user's latest KYC status (`PENDING`, `VERIFIED`, `REJECTED`, `PENDING_REVIEW`).     |
| `sortBy`       | string | Field to sort by (`created_at`, `email`, `first_name`, `status`). Defaults to `created_at`.       |
| `sortDir`      | string | Sort direction (`asc`, `desc`). Defaults to `desc`.                                               |
| `page`         | int    | Page number (0-indexed). Defaults to `0`.                                                         |
| `limit`        | int    | Items per page. Defaults to `10`.                                                                 |

**Response:**
```json
{
  "users": [
    {
      "id": "uuid",
      "firstName": "John",
      "lastName": "Doe",
      "email": "john@example.com",
      "phoneNo": "1234567890",
      "userRole": "user",
      "associateType": null,
      "status": "active",
      "verified": true,
      "isVerifiedBroker": false,
      "kycStatus": "VERIFIED",
      "createdAt": "2026-06-15T12:00:00Z"
    }
  ],
  "total": 100,
  "page": 0,
  "limit": 10
}
```

---

## 2. Get User Detail (Compound Query)

**Endpoint:** `GET /api/admin/users/:id`

**Description:** Retrieves a user's basic information alongside their latest KYC status, recent properties, bookings, chats, and related moderation reports.

**Path Parameters:**
- `id` (UUID): The target user's ID.

**Response:**
```json
{
  "success": true,
  "user": {
    "id": "uuid",
    "firstName": "John",
    "lastName": "Doe",
    "email": "john@example.com",
    "phoneNo": "1234567890",
    "userRole": "user",
    "associateType": null,
    "status": "active",
    "verified": true,
    "isVerifiedBroker": false,
    "createdAt": "2026-06-15T12:00:00Z"
  },
  "latestKyc": {
    "status": "VERIFIED",
    "createdAt": "2026-06-16T12:00:00Z"
  },
  "recentProperties": [
    { "id": "uuid", "title": "2BHK Apartment", "status": "active", "createdAt": "..." }
  ],
  "recentBookings": [
    { "id": "uuid", "providerId": "uuid", "status": "completed", "scheduledAt": "...", "createdAt": "..." }
  ],
  "recentChats": [
    { "id": "uuid", "name": "Chat with Alex", "createdAt": "..." }
  ],
  "relatedReports": [
    { "id": "uuid", "entityType": "USER", "entityId": "uuid", "reason": "spam", "status": "PENDING_REVIEW", "createdAt": "..." }
  ]
}
```

---

## 3. Update User Attributes

**Endpoint:** `PATCH /api/admin/users/:id`

**Description:** Partially updates a user's mutable fields.

**Request Body:**
```json
{
  "status": "suspended",
  "userRole": "broker",
  "associateType": "agency",
  "isVerifiedBroker": true,
  "email": "new.email@example.com"
}
```
*(All fields are optional)*

**Response:**
```json
{
  "success": true,
  "message": "User updated successfully"
}
```

---

## 4. Suspend User

**Endpoint:** `POST /api/admin/users/:id/suspend`

**Description:** Changes a user's status to `suspended` and securely logs the reason in the `admin_audit_logs`.

**Request Body:**
```json
{
  "reason": "Violated terms of service (minimum 10 characters)."
}
```

**Response:**
```json
{
  "success": true,
  "message": "User suspended"
}
```

---

## 5. Reinstate User

**Endpoint:** `POST /api/admin/users/:id/reinstate`

**Description:** Changes a suspended user's status back to `active` and logs the action in the `admin_audit_logs`.

**Response:**
```json
{
  "success": true,
  "message": "User reinstated"
}
```

---

## 6. Force Delete User

**Endpoint:** `DELETE /api/admin/users/:id/force`

**Description:** **Irreversible operation**. Recursively drops all user traces across `properties`, `kyc`, `chats`, `messages`, `news`, `bookings`, `reports`, `reviews`, `saved_properties`, and `notifications`. Returns the count of deleted entities per table.

**Response:**
```json
{
  "success": true,
  "message": "User force deleted",
  "counts": {
    "properties": 3,
    "kyc": 1,
    "chats": 5,
    "messages": 24,
    "news": 0,
    "bookings": 2,
    "reports": 0,
    "reviews": 1,
    "saved_rows": 12,
    "notifications": 35,
    "user": 1
  }
}
```

---

## 7. Bulk Action

**Endpoint:** `POST /api/admin/users/bulk-action`

**Description:** Applies a single administrative action (`suspend`, `reinstate`, or `force-delete`) across an array of users atomically inside a database transaction.

**Request Body:**
```json
{
  "userIds": ["uuid1", "uuid2"],
  "action": "suspend", 
  "reason": "Temporary suspension for suspicious activity"
}
```

*Valid `action` values: `suspend`, `reinstate`, `force-delete`*

**Response:**
```json
{
  "success": true,
  "message": "Bulk action completed successfully"
}
```
