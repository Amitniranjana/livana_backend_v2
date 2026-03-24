# 🏠 Livana Backend — Frontend Integration Guide
> **Base URL:** `http://localhost:9090` (dev) | `https://api.livana.co` (prod)
> **Version:** v1 | **Last Updated:** 2026-02-23

---

## 📋 Table of Contents
1. [Standard Response Format](#standard-response-format)
2. [Authentication](#authentication)
3. [Property Search API](#1-property-search-api)
4. [Filters API](#2-filters-api)
5. [Suggestions / Autocomplete API](#3-suggestions--autocomplete-api)
6. [CareCrew APIs](#4-carecrew-apis)
7. [Error Codes](#error-codes)

---

## Standard Response Format

Every API response follows this structure:

### ✅ Success
```json
{
  "success": true,
  "message": "Human readable message",
  "data": { }
}
```

### ❌ Error
```json
{
  "success": false,
  "message": "What went wrong",
  "error_code": "SNAKE_CASE_CODE",
  "errors": ["optional detail"]
}
```

### 📄 Paginated List
```json
{
  "success": true,
  "message": "...",
  "data": {
    "items": [],
    "pagination": {
      "total_count": 100,
      "current_page": 1,
      "total_pages": 10,
      "limit": 10,
      "offset": 0
    }
  }
}
```

---

## Authentication

Authenticated endpoints require a Bearer token in the `Authorization` header.

```http
Authorization: Bearer <jwt_token>
```

Get the token by calling `POST /api/auth/signin`. Endpoints marked 🔒 require this header.

---

### `POST /api/auth/signup`

Registers a new user account and returns an authentication token along with the user details.

**Auth:** Not required

#### Request Body
```json
{
  "firstName": "John",
  "lastName": "Doe",
  "email": "john.doe@example.com",
  "password": "password123",
  "phoneNo": "1234567890",
  "gender": "male",
  "userRole": "user"
}
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `firstName` | string | ✅ | |
| `lastName` | string | ✅ | |
| `email` | string | ❌ | Optional email address |
| `password` | string | ✅ | |
| `phoneNo` | string | ✅ | |
| `gender` | string | ✅ | e.g. "male", "female", "other" |
| `userRole` | string | ❌ | Defaults to `user` if not provided |
| `businessName` | string | ❌ | Optional |
| `licenseNumber` | string | ❌ | Optional |
| `experienceYears`| integer| ❌ | Optional |
| `commissionRate` | float  | ❌ | Optional |

> **Note:** `associate_type` is dynamically handled by the backend and set to `null` to establish base signup logic, and does not need to be provided in the payload.

#### Success Response `201 Created`
```json
{
  "success": true,
  "message": "User created successfully. Verification OTP sent to email and phone.",
  "data": {
    "token": "eyJhbGciOiJIUzI1...",
    "user": {
      "id": "uuid",
      "first_name": "John",
      "last_name": "Doe",
      "email": "john.doe@example.com",
      "phone_no": "1234567890",
      "user_role": "user",
      "verified": false,
      "status": "active",
      "associate_type": null,
      "created_at": "2026-03-03T10:00:00Z"
    }
  }
}
```

#### Error Responses
| Code | HTTP | When |
|------|------|------|
| `BAD_REQUEST` | 400 | Data validation failed or payload invalid |
| `CONFLICT` | 409 | User with this email or phone number already exists |

---

## 1. Property Search API

### `GET /api/v1/properties/search`

Search properties with filters, sorting, and pagination. Works like Housing.com search.

#### Query Parameters

| Parameter | Type | Required | Example | Description |
|-----------|------|----------|---------|-------------|
| `q` | string | ❌ | `Sector 62` | Free-text search — searches across city, locality, project, builder, landmark |
| `city` | string | ❌ | `Noida` | Filter by city |
| `locality` | string | ❌ | `Sector 62` | Filter by locality |
| `pincode` | string | ❌ | `201309` | Filter by pincode |
| `minPrice` | integer | ❌ | `2000000` | Minimum price in ₹ |
| `maxPrice` | integer | ❌ | `8000000` | Maximum price in ₹ |
| `bhk` | string | ❌ | `2,3` | BHK count — comma-separated for multi-select |
| `propertyType` | string | ❌ | `flat,villa` | flat \| villa \| plot \| commercial |
| `furnishing` | string | ❌ | `semi,fully` | unfurnished \| semi \| fully |
| `minArea` | integer | ❌ | `800` | Minimum area in sq.ft |
| `maxArea` | integer | ❌ | `2000` | Maximum area in sq.ft |
| `amenities` | string | ❌ | `lift,parking,gym` | Filter by amenities (AND logic — all must match) |
| `postedBy` | string | ❌ | `owner,broker` | owner \| broker \| builder |
| `sort` | string | ❌ | `price_asc` | `relevance` \| `newest` \| `price_asc` \| `price_desc` |
| `page` | integer | ❌ | `1` | Page number (default: 1) |
| `limit` | integer | ❌ | `10` | Results per page — max 100 (default: 10) |

#### Example Request
```http
GET /api/v1/properties/search?city=Noida&locality=Sector 62&minPrice=2000000&maxPrice=8000000&bhk=2,3&propertyType=flat&furnishing=semi,fully&minArea=800&maxArea=2000&amenities=lift,parking&sort=price_asc&page=1&limit=10
```

#### Success Response `200 OK`
```json
{
  "success": true,
  "message": "Properties retrieved successfully",
  "data": {
    "properties": [
      {
        "propertyId": "550e8400-e29b-41d4-a716-446655440000",
        "title": "2 BHK Flat in Sector 62",
        "price": 5500000,
        "priceLabel": "₹55 Lakh",
        "bhk": 2,
        "propertyType": "flat",
        "area": 1050,
        "furnishing": "semi",
        "availability": "ready",
        "address": {
          "city": "Noida",
          "locality": "Sector 62",
          "pincode": "201309",
          "landmark": "Near Metro Station"
        },
        "geo": {
          "lat": 28.6274,
          "lng": 77.3653
        },
        "amenities": ["lift", "parking", "gym"],
        "images": ["https://cdn.livana.co/img/prop1.jpg"],
        "primaryImage": "https://cdn.livana.co/img/prop1.jpg",
        "isVerified": true,
        "isFeatured": false,
        "postedBy": "owner",
        "projectName": "Gaur City",
        "builderName": "Gaursons",
        "createdAt": "2026-02-10T08:30:00Z"
      }
    ],
    "pagination": {
      "total_count": 48,
      "current_page": 1,
      "total_pages": 5,
      "limit": 10,
      "offset": 0
    }
  }
}
```

---

## 2. Filters API

### `GET /api/v1/properties/filters`

Returns all available filter options and dynamic price/area ranges for a given city or query. Call this when the filter panel opens.

#### Query Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `city` | string | ❌ | Get ranges contextual to a city |
| `q` | string | ❌ | Alternative to `city` |

#### Example Request
```http
GET /api/v1/properties/filters?city=Noida
```

#### Success Response `200 OK`
```json
{
  "success": true,
  "message": "Filter options retrieved successfully",
  "data": {
    "filters": {
      "priceRange": {
        "min": 500000,
        "max": 50000000
      },
      "areaRange": {
        "min": 200,
        "max": 5000
      },
      "bhkOptions": [
        { "value": 1, "label": "1 BHK" },
        { "value": 2, "label": "2 BHK" },
        { "value": 3, "label": "3 BHK" },
        { "value": 4, "label": "4 BHK" },
        { "value": 5, "label": "5+ BHK" }
      ],
      "propertyTypes": [
        { "value": "flat",        "label": "Flat / Apartment" },
        { "value": "villa",       "label": "Villa / Independent House" },
        { "value": "plot",        "label": "Plot / Land" },
        { "value": "commercial",  "label": "Commercial" }
      ],
      "furnishingOptions": [
        { "value": "unfurnished", "label": "Unfurnished" },
        { "value": "semi",        "label": "Semi-Furnished" },
        { "value": "fully",       "label": "Fully Furnished" }
      ],
      "availabilityOptions": [
        { "value": "ready",                "label": "Ready to Move" },
        { "value": "under_construction",   "label": "Under Construction" }
      ],
      "amenitiesOptions": [
        { "value": "lift",           "label": "Lift" },
        { "value": "parking",        "label": "Parking" },
        { "value": "gym",            "label": "Gym" },
        { "value": "pool",           "label": "Swimming Pool" },
        { "value": "security",       "label": "24x7 Security" },
        { "value": "power_backup",   "label": "Power Backup" },
        { "value": "garden",         "label": "Garden" },
        { "value": "clubhouse",      "label": "Clubhouse" }
      ],
      "postedByOptions": [
        { "value": "owner",   "label": "Owner" },
        { "value": "broker",  "label": "Broker" },
        { "value": "builder", "label": "Builder" }
      ],
      "sortOptions": [
        { "value": "relevance",  "label": "Relevance" },
        { "value": "newest",     "label": "Newest First" },
        { "value": "price_asc",  "label": "Price: Low to High" },
        { "value": "price_desc", "label": "Price: High to Low" }
      ]
    }
  }
}
```

---

## 3. Suggestions / Autocomplete API

### `GET /api/v1/search/suggestions`

Returns fast, categorized autocomplete suggestions as the user types. Minimum 2 characters required.

#### Query Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `q` | string | ✅ | At least 2 characters |

#### Example Request
```http
GET /api/v1/search/suggestions?q=sec
```

#### Success Response `200 OK`
```json
{
  "success": true,
  "message": "Suggestions retrieved successfully",
  "data": {
    "suggestions": [
      { "category": "locality", "value": "Sector 62, Noida" },
      { "category": "locality", "value": "Sector 44, Gurugram" },
      { "category": "city",     "value": "Secunderabad" },
      { "category": "project",  "value": "Sector 150 Express View" },
      { "category": "builder",  "value": "Sector Homes Corp" },
      { "category": "landmark", "value": "Sector 18 Market" }
    ]
  }
}
```

**Category values:** `city` | `locality` | `project` | `builder` | `landmark`

#### Validation Error `400 Bad Request`
```json
{
  "success": false,
  "message": "Query must be at least 2 characters",
  "error_code": "QUERY_TOO_SHORT"
}
```

---

## 4. CareCrew APIs

CareCrew is a home-services module (like Urban Company) — users can browse services, find providers, and book appointments.

---

### `GET /api/v1/carecrew/services`

List all available CareCrew services.

**Auth:** Not required

#### Success Response `200 OK`
```json
{
  "success": true,
  "message": "CareCrew services retrieved successfully",
  "data": {
    "services": [
      {
        "id": "uuid",
        "name": "Plumbing",
        "description": "Fix leaks, pipe repairs, bathroom fittings",
        "iconUrl": null,
        "category": "home_repair",
        "isActive": true
      },
      { "id": "uuid", "name": "Electrical",  "description": "Wiring, switches, MCB replacement",              "iconUrl": null, "category": "home_repair", "isActive": true },
      { "id": "uuid", "name": "Cleaning",    "description": "Deep cleaning, sofa cleaning, pest control",    "iconUrl": null, "category": "cleaning",    "isActive": true },
      { "id": "uuid", "name": "Painting",    "description": "Interior and exterior painting",                "iconUrl": null, "category": "renovation",  "isActive": true },
      { "id": "uuid", "name": "Carpentry",   "description": "Furniture repair, wardrobes, modular kitchen",  "iconUrl": null, "category": "home_repair", "isActive": true }
    ],
    "total": 5
  }
}
```

---

### `GET /api/v1/carecrew/services/{id}`

Get a single service by ID.

**Auth:** Not required

**Errors:** `400 INVALID_UUID` | `404 NOT_FOUND`

---

### `GET /api/v1/carecrew/providers`

Search and browse service providers.

**Auth:** Not required

#### Query Parameters

| Parameter | Type | Example | Description |
|-----------|------|---------|-------------|
| `serviceType` | string | `plumbing` | Filter by service type |
| `city` | string | `Mumbai` | Filter by city |
| `page` | integer | `1` | Default: 1 |
| `limit` | integer | `10` | Max: 50. Default: 10 |

#### Example Request
```http
GET /api/v1/carecrew/providers?serviceType=plumbing&city=Mumbai&page=1&limit=10
```

#### Success Response `200 OK`
```json
{
  "success": true,
  "message": "Providers retrieved successfully",
  "data": {
    "providers": [
      {
        "id": "uuid",
        "name": "Ramesh Kumar",
        "bio": "10+ years experience in plumbing across Mumbai",
        "serviceType": "plumbing",
        "city": "Mumbai",
        "rating": 4.8,
        "reviewCount": 124,
        "isFeatured": true,
        "avatarUrl": "https://cdn.livana.co/avatars/ramesh.jpg",
        "phone": "+91XXXXXXXXXX"
      }
    ],
    "pagination": {
      "total_count": 15,
      "current_page": 1,
      "total_pages": 2,
      "limit": 10
    }
  }
}
```

---

### `GET /api/v1/carecrew/providers/featured`

Returns top 10 featured providers sorted by rating.

**Auth:** Not required

---

### `GET /api/v1/carecrew/providers/{id}`

Get a provider's full profile.

**Auth:** Not required

**Errors:** `400 INVALID_UUID` | `404 NOT_FOUND`

---

### `POST /api/v1/carecrew/bookings` 🔒

Create a new booking with a provider.

**Auth:** Required (`Authorization: Bearer <token>`)

#### Request Body
```json
{
  "provider_id": "uuid",
  "service_id": "uuid",
  "scheduled_at": "2026-03-15T10:00:00Z",
  "notes": "Please come in the morning"
}
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `provider_id` | UUID string | ✅ | Must be an active provider |
| `service_id` | UUID string | ✅ | Must be an active service |
| `scheduled_at` | string | ✅ | ISO 8601 format: `YYYY-MM-DDTHH:MM:SSZ` |
| `notes` | string | ❌ | Optional message to provider |

#### Success Response `201 Created`
```json
{
  "success": true,
  "message": "Booking created successfully",
  "data": {
    "booking": {
      "id": "uuid",
      "providerId": "uuid",
      "serviceId": "uuid",
      "userId": "uuid",
      "scheduledAt": "2026-03-15T10:00:00Z",
      "status": "pending",
      "notes": "Please come in the morning",
      "createdAt": "2026-02-23T17:55:00Z"
    }
  }
}
```

#### Error Responses
| Code | HTTP | When |
|------|------|------|
| `UNAUTHORIZED` | 401 | No/invalid Bearer token |
| `INVALID_UUID` | 400 | Malformed UUID in body |
| `INVALID_DATETIME` | 400 | `scheduled_at` not ISO 8601 |
| `PROVIDER_NOT_FOUND` | 404 | Provider UUID doesn't exist |
| `SERVICE_NOT_FOUND` | 404 | Service UUID doesn't exist |

---

### `PUT /api/v1/carecrew/bookings/{id}/status` 🔒

Update the status of a booking.

**Auth:** Required

#### Request Body
```json
{ "status": "confirmed" }
```

#### Booking Status Flow
```
pending ──► confirmed ──► in_progress ──► completed
  │              │               │
  ▼              ▼               ▼
cancelled    cancelled       cancelled
```

> ⚠️ `completed` and `cancelled` are **final** — cannot be changed again.

| From | To | Allowed? |
|------|----|----------|
| `pending` | `confirmed` | ✅ |
| `pending` | `cancelled` | ✅ |
| `confirmed` | `in_progress` | ✅ |
| `confirmed` | `cancelled` | ✅ |
| `in_progress` | `completed` | ✅ |
| `in_progress` | `cancelled` | ✅ |
| `completed` | anything | ❌ |
| `cancelled` | anything | ❌ |
| `pending` | `completed` | ❌ (must go step by step) |

#### Success Response `200 OK`
```json
{
  "success": true,
  "message": "Booking status updated successfully",
  "data": {
    "booking": { "id": "uuid", "status": "confirmed", "..." : "..." }
  }
}
```

#### Error Responses
| Code | HTTP | When |
|------|------|------|
| `INVALID_STATUS` | 400 | Status value not in allowed list |
| `INVALID_TRANSITION` | 422 | E.g. `pending → completed` |
| `NOT_FOUND` | 404 | Booking ID doesn't exist |

---

### `GET /api/v1/carecrew/providers/{id}/bookings` 🔒

Get all bookings for a provider. Includes customer name, email, phone.

**Auth:** Required

#### Query Parameters
| Parameter | Default | Max |
|-----------|---------|-----|
| `page` | 1 | — |
| `limit` | 10 | 50 |

#### Success Response `200 OK`
```json
{
  "success": true,
  "message": "Bookings retrieved successfully",
  "data": {
    "bookings": [
      {
        "id": "uuid",
        "providerId": "uuid",
        "serviceId": "uuid",
        "userId": "uuid",
        "scheduledAt": "2026-03-15T10:00:00Z",
        "status": "pending",
        "notes": "Morning preferred",
        "createdAt": "2026-02-23T17:55:00Z",
        "user": {
          "name": "Ravi Sharma",
          "email": "ravi@example.com",
          "phone": "+91XXXXXXXXXX"
        }
      }
    ],
    "pagination": {
      "total_count": 8,
      "current_page": 1,
      "total_pages": 1,
      "limit": 10
    }
  }
}
```

---

## Error Codes

| Error Code | HTTP | Description |
|------------|------|-------------|
| `UNAUTHORIZED` | 401 | Missing or invalid Bearer token |
| `INVALID_TOKEN` | 401 | JWT signature invalid or expired |
| `INVALID_UUID` | 400 | UUID in path or body is malformed |
| `QUERY_TOO_SHORT` | 400 | Suggestions `q` param < 2 characters |
| `INVALID_DATETIME` | 400 | `scheduled_at` not ISO 8601 format |
| `INVALID_STATUS` | 400 | Booking status not a known value |
| `INVALID_TRANSITION` | 422 | Booking state machine transition not allowed |
| `NOT_FOUND` | 404 | Resource does not exist |
| `PROVIDER_NOT_FOUND` | 404 | Provider UUID not active/found |
| `SERVICE_NOT_FOUND` | 404 | Service UUID not active/found |
| `SEARCH_DB_ERROR` | 500 | DB error on property search |
| `FILTERS_DB_ERROR` | 500 | DB error on filters fetch |
| `SUGGESTIONS_DB_ERROR` | 500 | DB error on suggestions fetch |
| `DB_ERROR` | 500 | General database error |

---

## Quick Reference — All New Endpoints

| Method | Endpoint | Auth | Description |
|--------|----------|------|-------------|
| GET | `/api/v1/properties/search` | ❌ | Search properties with filters |
| GET | `/api/v1/properties/filters` | ❌ | Get dynamic filter options |
| GET | `/api/v1/search/suggestions` | ❌ | Autocomplete suggestions |
| GET | `/api/v1/carecrew/services` | ❌ | List all services |
| GET | `/api/v1/carecrew/services/{id}` | ❌ | Get service by ID |
| GET | `/api/v1/carecrew/providers` | ❌ | Search providers |
| GET | `/api/v1/carecrew/providers/featured` | ❌ | Featured providers |
| GET | `/api/v1/carecrew/providers/{id}` | ❌ | Provider detail |
| POST | `/api/v1/carecrew/bookings` | 🔒 | Create booking |
| PUT | `/api/v1/carecrew/bookings/{id}/status` | 🔒 | Update booking status |
| GET | `/api/v1/carecrew/providers/{id}/bookings` | 🔒 | Provider's bookings |
| GET | `/api/chats` | 🔒 | Retrieve all user chats (See [Chat Docs](Chat_API_Documentation.md)) |
