# Property Listing API

## Overview
The Listing API provides endpoints to create, retrieve, update, delete, and search properties in the Livana application. It handles advanced filtering and tracks exactly who posted the property (e.g., whether the user is a regular `user` or an `associate`).

## Tech Stack
- **Database:** PostgreSQL (SQLx)
- **Framework:** Axum (Rust)
- **OpenAPI/Swagger Docs:** Configured via `utoipa` on the Request DTOs.

## API Endpoints

### 1. Create a Property Listing
- **Endpoint:** `POST /api/properties`
- **Auth:** Required (JWT)
- **Payload (`CreatePropertyRequest`):**
  ```json
  {
    "title": "Modern 2BHK Apartment in City Center",
    "description": "Beautiful 2BHK apartment with modern amenities.",
    "property_type": "rent",
    "price": 25000,
    "deposit": 50000,
    "location": "Bandra West, Mumbai",
    ...
    "user_type": "user"    // Can be "user" or "associate"
  }
  ```
- **Description:** Creates a new property listing. If the `user_type` (e.g. `user` or `associate`) is provided, it is securely saved to the database.

### 2. Update a Property
- **Endpoint:** `PUT /api/properties/{id}`
- **Auth:** Required (JWT) - User must be the owner.
- **Payload (`UpdatePropertyRequest`):** (All fields are optional)
  ```json
  {
    "price": 24000,
    "user_type": "associate"
  }
  ```
- **Description:** Updates the fields of an existing property listing. You can change multiple fields at once, including updating the `user_type`.

### 3. Get All Properties (Paginated)
- **Endpoint:** `GET /api/properties?limit=20&offset=0&property_type=rent&sort_by=popular`
- **Auth:** Optional
- **Description:** Fetches an ordered, paginated list of all active properties. The returned JSON objects include the `user_type` so the frontend knows who listed it.

### 4. Search and Filter Properties
- **Endpoint:** `GET /api/properties/search`
- **Auth:** Optional
- **Query Parameters:** `query`, `min_price`, `max_price`, `bedrooms`, `location`, `property_type`, etc.
- **Description:** Advanced full-text and parameterized search of properties.

### 5. Get Owner's Properties
- **Endpoint:** `GET /api/properties/broker?status=active`
- **Auth:** Required (JWT)
- **Description:** Returns all properties posted by the currently authenticated user.

### 6. Delete a Property (Soft Delete)
- **Endpoint:** `DELETE /api/properties/{id}`
- **Auth:** Required (JWT) - User must be the owner.
- **Description:** Soft-deletes a property by changing its `status` to `deleted` (it remains in the database but won't be visible in public searches).

## Database Schema Highlights
**Table Name:** `listings`
- `id`: UUID (Primary Key)
- `title`, `description`, `price`: Property details
- `user_id`: UUID (Foreign Key to `users.id` - The Owner)
- **`user_type`**: String (`user`, `associate`) - Tracks the role under which the listing was posted.
- `status`: String (`active`, `inactive`, `deleted`)
