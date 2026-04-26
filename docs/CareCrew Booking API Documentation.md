# CareCrew Booking System API Documentation

This document outlines the API endpoints for the CareCrew Booking System. All endpoints require authentication via a JWT Bearer token in the `Authorization` header.

**Base URL:** `http://<your-backend-url>`
**Authentication:** `Authorization: Bearer <your-jwt-token>`

---

## 1. Get User Bookings
Retrieves a paginated list of bookings for the currently logged-in user.

**Endpoint:**
`GET /api/bookings`

**Query Parameters:**
| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `status` | `string` | ❌ No | - | Filter by status (`pending`, `confirmed`, `in_progress`, `completed`, `cancelled`) |
| `limit` | `integer`| ❌ No | `10` | Number of items per page (max 50) |
| `offset`| `integer`| ❌ No | `0` | Number of items to skip |

**Example Request:**
`GET /api/bookings?status=pending&limit=10&offset=0`

**Example Response:**
```json
{
  "success": true,
  "message": "Bookings retrieved successfully",
  "data": {
    "bookings": [
      {
        "booking_id": "550e8400-e29b-41d4-a716-446655440000",
        "booking_number": "BKG20260301A1B2C3",
        "provider_id": "provider-uuid",
        "provider_name": "John Doe",
        "provider_image": "https://example.com/avatar.jpg",
        "service_type": "Plumbing",
        "scheduled_date_time": "2026-03-05T10:00:00Z",
        "status": "pending",
        "address": "123 Main St",
        "estimated_cost": 500.0,
        "created_at": "2026-03-01T08:00:00Z"
      }
    ],
    "total_count": 1,
    "current_page": 1,
    "total_pages": 1
  }
}
```

---

## 2. Get Booking Details
Retrieves detailed information about a specific booking, including the tracking timeline. Can only be accessed by the booking owner or the assigned provider.

**Endpoint:**
`GET /api/bookings/{booking_id}`

**Example Response:**
```json
{
  "success": true,
  "message": "Booking details retrieved successfully",
  "data": {
    "booking_id": "550e8400-e29b-41d4-a716-446655440000",
    "booking_number": "BKG20260301A1B2C3",
    "provider_id": "provider-uuid",
    "provider_name": "John Doe",
    "provider_phone": "1234567890",
    "provider_image": "https://example.com/avatar.jpg",
    "provider_rating": 4.5,
    "service_type": "Plumbing",
    "scheduled_date_time": "2026-03-05T10:00:00Z",
    "status": "pending",
    "address": "123 Main St",
    "problem_description": "Leaking tap",
    "contact_number": "0987654321",
    "estimated_cost": 500.0,
    "final_cost": null,
    "payment_status": "pending",
    "tracking_status": [
      {
        "status": "pending",
        "timestamp": "2026-03-01T08:00:00Z",
        "description": "Your booking has been placed successfully"
      }
    ],
    "cancelled_at": null,
    "cancellation_reason": null,
    "created_at": "2026-03-01T08:00:00Z",
    "updated_at": null
  }
}
```

---

## 3. Get Provider Bookings
Retrieves a paginated list of bookings assigned to the currently logged-in provider.

**Endpoint:**
`GET /api/bookings/provider`

**Query Parameters:**
| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `status` | `string` | ❌ No | - | Filter by status |
| `limit` | `integer`| ❌ No | `10` | Number of items per page |
| `offset`| `integer`| ❌ No | `0` | Number of items to skip |

**Example Response:**
```json
{
  "success": true,
  "message": "Provider bookings retrieved successfully",
  "data": {
    "bookings": [
      {
        "booking_id": "550e8400-e29b-41d4-a716-446655440000",
        "booking_number": "BKG20260301A1B2C3",
        "customer_name": "Jane Smith",
        "customer_phone": "0987654321",
        "customer_image": null,
        "service_type": "Plumbing",
        "scheduled_date_time": "2026-03-05T10:00:00Z",
        "status": "pending",
        "address": "123 Main St",
        "problem_description": "Leaking tap",
        "estimated_cost": 500.0,
        "created_at": "2026-03-01T08:00:00Z"
      }
    ],
    "total_count": 1,
    "current_page": 1,
    "total_pages": 1
  }
}
```

---

## 4. Update Booking Status
Updates the status of a booking. Valid state transitions are enforced.

**Endpoint:**
`PUT /api/bookings/{booking_id}/status`

**Valid Transitions:**
- `pending` → `confirmed` or `cancelled`
- `confirmed` → `in_progress` or `cancelled`
- `in_progress` → `completed` or `cancelled`

**Request Body:**
```json
{
  "status": "confirmed",
  "notes": "On my way",
  "estimated_cost": 600.0
}
```

**Example Response:**
```json
{
  "success": true,
  "message": "Booking status updated successfully",
  "data": {
    "booking": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "providerId": "provider-uuid",
      "serviceId": "service-uuid",
      "userId": "user-uuid",
      "scheduledAt": "2026-03-05T10:00:00Z",
      "status": "confirmed",
      "notes": "On my way",
      "createdAt": "2026-03-01T08:00:00Z"
    }
  }
}
```

---

## 5. Cancel Booking
Soft deletes a booking. Can only be done if the current status is `pending` or `confirmed`.

**Endpoint:**
`PUT /api/bookings/{booking_id}/cancel`

**Request Body:**
```json
{
  "cancellation_reason": "Changed plan"
}
```

**Example Response:**
```json
{
  "success": true,
  "message": "Booking cancelled successfully",
  "data": {
    "booking": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "providerId": "provider-uuid",
      "serviceId": "service-uuid",
      "userId": "user-uuid",
      "scheduledAt": "2026-03-05T10:00:00Z",
      "status": "cancelled",
      "notes": null,
      "createdAt": "2026-03-01T08:00:00Z"
    }
  }
}
```

---

## Standard Error Codes

| Error Code | Description | HTTP Status |
|---|---|---|
| `UNAUTHORIZED` | Missing or invalid token | 401 |
| `ACCESS_DENIED` | You are not the owner/provider | 403 |
| `BOOKING_NOT_FOUND` | Booking does not exist | 404 |
| `INVALID_STATUS` | Status is not one of the allowed values | 400 |
| `INVALID_STATUS_TRANSITION` | The state transition is invalid | 422 |
| `BOOKING_CANNOT_BE_CANCELLED` | Trying to cancel a completed/in-progress booking | 400 |
| `DB_ERROR` | Internal database failure | 500 |
