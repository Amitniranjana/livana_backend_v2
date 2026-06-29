# Admin Properties API Documentation

This document outlines the Admin Properties endpoints that have been added for the frontend team. All endpoints are prefixed with `/api/admin/properties` and require an administrative authentication token (passed as a Bearer token in the `Authorization` header).

---

## Base URL
`/api/admin/properties`

## Authentication
All endpoints require a valid admin JWT token.
- **Header:** `Authorization: Bearer <token>`

---

## 1. Get Paginated Properties List

Retrieve a paginated list of properties with robust filtering and sorting options.

- **Method:** `GET`
- **Endpoint:** `/api/admin/properties`
- **Query Parameters:**
  - `limit` (integer, default: 20): Number of results per page (max 100).
  - `offset` (integer, default: 0): Pagination offset.
  - `search` (string): Search query matching `title`, `city`, `locality`, or owner `first_name`/`last_name`.
  - `status` (string): Filter by property status (e.g., `active`, `inactive`, `deleted`).
  - `listingType` (string): Filter by listing type.
  - `propertyType` (string): Filter by property type (e.g., `flat`, `villa`, `commercial`).
  - `city` (string): Filter by exact city name.
  - `isFeatured` (boolean): `true` to fetch only featured properties, `false` otherwise.
  - `postedByUserType` (string): Filter by user type who posted it (e.g., `owner`, `broker`, `builder`).
  - `userId` (UUID): Fetch properties posted by a specific user.
  - `sortBy` (string): Field to sort by (e.g., `created_at`, `price`, `title`). Defaults to `created_at`.
  - `sortDir` (string): Direction to sort, `asc` or `desc`. Defaults to `desc`.

**Example Request:**
```http
GET /api/admin/properties?limit=10&offset=0&status=active&sortBy=price&sortDir=asc
```

**Success Response (200 OK):**
```json
{
  "success": true,
  "data": [
    {
      "id": "e4b6c33c-3e61-419f-b98a-2c8c4eb5f7a0",
      "title": "Beautiful 2BHK in Downtown",
      "description": "...",
      "property_type": "flat",
      "listing_type": "rent",
      "price": 45000,
      "location": "Downtown",
      "city": "Mumbai",
      "status": "active",
      "is_featured": true,
      "is_verified": true,
      "user_type": "owner",
      "images": ["url1", "url2"],
      "created_at": "2026-06-29T10:00:00Z",
      "owner": {
        "id": "a1b2c3d4-e5f6...",
        "name": "John Doe"
      }
    }
  ],
  "pagination": {
    "total": 145,
    "limit": 10,
    "offset": 0
  }
}
```

---

## 2. Get Property Detail

Retrieve full details of a specific property, including the owner's information and all associated property reviews.

- **Method:** `GET`
- **Endpoint:** `/api/admin/properties/:id`
- **Path Parameters:**
  - `id` (UUID): The unique identifier of the property.

**Success Response (200 OK):**
```json
{
  "success": true,
  "data": {
    "id": "e4b6c33c-...",
    "title": "Beautiful 2BHK in Downtown",
    "price": 45000,
    "status": "active",
    "is_featured": true,
    "is_verified": true,
    "reviews": [
      {
        "id": "...",
        "rating": 5,
        "review": "Great location and well maintained.",
        "created_at": "2026-06-25T12:00:00Z",
        "user_name": "Jane Smith"
      }
    ],
    // ...other property fields
  }
}
```

**Error Response (404 Not Found):**
```json
{
  "success": false,
  "message": "Property not found"
}
```

---

## 3. Update Property

Update specific attributes of a property (specifically used for moderation like featuring, verifying, or changing status).

- **Method:** `PATCH`
- **Endpoint:** `/api/admin/properties/:id`
- **Path Parameters:**
  - `id` (UUID): The unique identifier of the property.
- **Request Body:** (All fields are optional)
```json
{
  "status": "inactive",
  "isFeatured": true,
  "isVerified": false
}
```

**Success Response (200 OK):**
```json
{
  "success": true,
  "message": "Property updated successfully"
}
```

---

## 4. Force Delete Property

Forcefully delete a property and cascade the deletion to all its related dependencies. This securely deletes rows from `property_reviews`, `saved_properties`, `site_visits`, `moderation_reports`, and `listing_images` related to this property before deleting the property itself. The action is securely logged in the system's audit trail.

- **Method:** `DELETE`
- **Endpoint:** `/api/admin/properties/:id/force`
- **Path Parameters:**
  - `id` (UUID): The unique identifier of the property.

**Success Response (200 OK):**
```json
{
  "success": true,
  "message": "Property and all related data forcefully deleted",
  "cascadeDeleted": true
}
```

---

## 5. Bulk Action Properties

Perform a bulk operation (feature, unfeature, suspend, or force-delete) on multiple properties at once.

- **Method:** `POST`
- **Endpoint:** `/api/admin/properties/bulk-action`
- **Request Body:**
  - `propertyIds` (Array of UUIDs): List of properties to apply the action to.
  - `action` (string): The action to perform. Valid values are:
    - `"feature"`: Sets `is_featured = true`.
    - `"unfeature"`: Sets `is_featured = false`.
    - `"suspend"`: Sets `status = 'inactive'`.
    - `"force-delete"`: Triggers a cascading deletion identical to the `DELETE /force` endpoint.
  - `reason` (string, optional): A reason for the audit log (mainly utilized for `force-delete`).

**Example Request:**
```json
{
  "propertyIds": [
    "e4b6c33c-3e61-419f-b98a-2c8c4eb5f7a0",
    "b8f2d590-7711-4148-8df0-64219a3b934c"
  ],
  "action": "suspend",
  "reason": "Violated terms of service"
}
```

**Success Response (200 OK):**
```json
{
  "success": true,
  "message": "Successfully processed 2/2 properties"
}
```
