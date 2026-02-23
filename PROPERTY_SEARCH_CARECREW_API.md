# Livana — Property Search & CareCrew API Docs
> Base URL: `http://localhost:9090` | Date: 2026-02-23

---

## Table of Contents
1. [Response Format](#response-format)
2. [Property Search](#property-search)
3. [Filters](#filters)
4. [Autocomplete / Suggestions](#autocomplete--suggestions)
5. [CareCrew Services](#carecrew-services)
6. [CareCrew Providers](#carecrew-providers)
7. [CareCrew Bookings](#carecrew-bookings)
8. [All Endpoints at a Glance](#all-endpoints)
9. [Error Codes](#error-codes)

---

## Response Format

**Every response follows this shape:**

```json
// Success
{ "success": true,  "message": "...", "data": {} }

// Error
{ "success": false, "message": "...", "error_code": "SNAKE_CODE", "errors": [] }

// List (paginated)
{
  "success": true, "message": "...",
  "data": {
    "items": [],
    "pagination": { "total_count": 100, "current_page": 1, "total_pages": 10, "limit": 10 }
  }
}
```

---

## Property Search

### `GET /api/v1/properties/search`

Main search endpoint. Works like Housing.com — supports free-text + all filters + sorting + pagination.

**Auth:** ❌ Not required

#### Query Params

| Param | Type | Example | Notes |
|-------|------|---------|-------|
| `q` | string | `Sector 62` | Free-text — searches city, locality, project, builder, landmark |
| `city` | string | `Noida` | Exact city filter |
| `locality` | string | `Sector 62` | Locality filter |
| `pincode` | string | `201309` | Pincode filter |
| `minPrice` | number | `2000000` | In ₹ |
| `maxPrice` | number | `8000000` | In ₹ |
| `bhk` | CSV | `2,3` | Multi-select: `1,2,3,4,5` |
| `propertyType` | CSV | `flat,villa` | `flat` \| `villa` \| `plot` \| `commercial` |
| `furnishing` | CSV | `semi,fully` | `unfurnished` \| `semi` \| `fully` |
| `minArea` | number | `800` | In sq.ft |
| `maxArea` | number | `2000` | In sq.ft |
| `amenities` | CSV | `lift,parking,gym` | AND logic — all must match |
| `postedBy` | CSV | `owner,broker` | `owner` \| `broker` \| `builder` |
| `sort` | string | `price_asc` | `relevance` \| `newest` \| `price_asc` \| `price_desc` |
| `page` | number | `1` | Default: 1 |
| `limit` | number | `10` | Default: 10, max: 100 |

#### Example
```http
GET /api/v1/properties/search?city=Noida&bhk=2,3&minPrice=2000000&maxPrice=8000000&amenities=lift,parking&sort=price_asc&page=1&limit=10
```

#### Response `200 OK`
```json
{
  "success": true,
  "message": "Properties retrieved successfully",
  "data": {
    "properties": [
      {
        "propertyId":    "550e8400-e29b-41d4-a716-446655440000",
        "title":         "2 BHK Flat in Sector 62",
        "price":         5500000,
        "priceLabel":    "₹55 Lakh",
        "bhk":           2,
        "propertyType":  "flat",
        "area":          1050,
        "furnishing":    "semi",
        "availability":  "ready",
        "address": {
          "city":     "Noida",
          "locality": "Sector 62",
          "pincode":  "201309",
          "landmark": "Near Metro Station"
        },
        "geo": { "lat": 28.6274, "lng": 77.3653 },
        "amenities":    ["lift", "parking", "gym"],
        "primaryImage": "https://cdn.livana.co/img/prop1.jpg",
        "images":       ["https://cdn.livana.co/img/prop1.jpg"],
        "isVerified":   true,
        "isFeatured":   false,
        "postedBy":     "owner",
        "projectName":  "Gaur City",
        "builderName":  "Gaursons",
        "createdAt":    "2026-02-10T08:30:00Z"
      }
    ],
    "pagination": {
      "total_count":  48,
      "current_page": 1,
      "total_pages":  5,
      "limit":        10,
      "offset":       0
    }
  }
}
```

---

## Filters

### `GET /api/v1/properties/filters`

Returns all filter options + dynamic price/area ranges. Call when filter panel opens.

**Auth:** ❌ Not required

#### Query Params

| Param | Example | Notes |
|-------|---------|-------|
| `city` | `Noida` | Optional — makes ranges contextual to city |

#### Example
```http
GET /api/v1/properties/filters?city=Noida
```

#### Response `200 OK`
```json
{
  "success": true,
  "message": "Filter options retrieved successfully",
  "data": {
    "filters": {
      "priceRange": { "min": 500000, "max": 50000000 },
      "areaRange":  { "min": 200,    "max": 5000 },
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
        { "value": "ready",               "label": "Ready to Move" },
        { "value": "under_construction",  "label": "Under Construction" }
      ],
      "amenitiesOptions": [
        { "value": "lift",          "label": "Lift" },
        { "value": "parking",       "label": "Parking" },
        { "value": "gym",           "label": "Gym" },
        { "value": "pool",          "label": "Swimming Pool" },
        { "value": "security",      "label": "24x7 Security" },
        { "value": "power_backup",  "label": "Power Backup" },
        { "value": "garden",        "label": "Garden" },
        { "value": "clubhouse",     "label": "Clubhouse" }
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

## Autocomplete / Suggestions

### `GET /api/v1/search/suggestions`

Categorized suggestions as the user types. Use for the search bar.

**Auth:** ❌ Not required | **Minimum query:** 2 characters

#### Query Params

| Param | Required | Example |
|-------|----------|---------|
| `q` | ✅ Yes | `sec` |

#### Example
```http
GET /api/v1/search/suggestions?q=sec
```

#### Response `200 OK`
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

**Possible `category` values:** `city` | `locality` | `project` | `builder` | `landmark`

#### Error — Query Too Short `400`
```json
{
  "success": false,
  "message": "Query must be at least 2 characters",
  "error_code": "QUERY_TOO_SHORT"
}
```

---

## CareCrew Services

### `GET /api/v1/carecrew/services`

List all available services (Plumbing, Electrical, Cleaning, etc.)

**Auth:** ❌ Not required

#### Response `200 OK`
```json
{
  "success": true,
  "message": "CareCrew services retrieved successfully",
  "data": {
    "services": [
      { "id": "uuid", "name": "Plumbing",   "description": "Fix leaks, pipe repairs, bathroom fittings",               "iconUrl": null, "category": "home_repair", "isActive": true },
      { "id": "uuid", "name": "Electrical", "description": "Wiring, switches, MCB replacement",                        "iconUrl": null, "category": "home_repair", "isActive": true },
      { "id": "uuid", "name": "Cleaning",   "description": "Deep cleaning, sofa cleaning, pest control",               "iconUrl": null, "category": "cleaning",    "isActive": true },
      { "id": "uuid", "name": "Painting",   "description": "Interior and exterior painting",                           "iconUrl": null, "category": "renovation",  "isActive": true },
      { "id": "uuid", "name": "Carpentry",  "description": "Furniture repair, wardrobes, modular kitchen fittings",    "iconUrl": null, "category": "home_repair", "isActive": true }
    ],
    "total": 5
  }
}
```

---

### `GET /api/v1/carecrew/services/{id}`

Get single service by ID.

**Auth:** ❌ Not required

**Errors:** `400 INVALID_UUID` | `404 NOT_FOUND`

---

## CareCrew Providers

### `GET /api/v1/carecrew/providers`

Search providers by service type and city.

**Auth:** ❌ Not required

#### Query Params

| Param | Example | Notes |
|-------|---------|-------|
| `serviceType` | `plumbing` | Optional filter |
| `city` | `Mumbai` | Optional filter |
| `page` | `1` | Default: 1 |
| `limit` | `10` | Default: 10, max: 50 |

#### Example
```http
GET /api/v1/carecrew/providers?serviceType=plumbing&city=Mumbai
```

#### Response `200 OK`
```json
{
  "success": true,
  "message": "Providers retrieved successfully",
  "data": {
    "providers": [
      {
        "id":           "uuid",
        "name":         "Ramesh Kumar",
        "bio":          "10+ years experience in plumbing across Mumbai",
        "serviceType":  "plumbing",
        "city":         "Mumbai",
        "rating":       4.8,
        "reviewCount":  124,
        "isFeatured":   true,
        "avatarUrl":    "https://cdn.livana.co/avatars/ramesh.jpg",
        "phone":        "+91XXXXXXXXXX"
      }
    ],
    "pagination": { "total_count": 15, "current_page": 1, "total_pages": 2, "limit": 10 }
  }
}
```

---

### `GET /api/v1/carecrew/providers/featured`

Top 10 featured providers sorted by rating.

**Auth:** ❌ Not required

---

### `GET /api/v1/carecrew/providers/{id}`

Full provider profile.

**Auth:** ❌ Not required

**Errors:** `400 INVALID_UUID` | `404 NOT_FOUND`

---

## CareCrew Bookings

### `POST /api/v1/carecrew/bookings` 🔒

Create a booking with a provider.

**Auth:** ✅ Required — `Authorization: Bearer <token>`

#### Request Body
```json
{
  "provider_id":  "uuid",
  "service_id":   "uuid",
  "scheduled_at": "2026-03-15T10:00:00Z",
  "notes":        "Please come in the morning"
}
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `provider_id` | UUID string | ✅ | Must be an active provider |
| `service_id` | UUID string | ✅ | Must be an active service |
| `scheduled_at` | ISO 8601 string | ✅ | Format: `2026-03-15T10:00:00Z` |
| `notes` | string | ❌ | Optional message for provider |

#### Response `201 Created`
```json
{
  "success": true,
  "message": "Booking created successfully",
  "data": {
    "booking": {
      "id":          "uuid",
      "providerId":  "uuid",
      "serviceId":   "uuid",
      "userId":      "uuid",
      "scheduledAt": "2026-03-15T10:00:00Z",
      "status":      "pending",
      "notes":       "Please come in the morning",
      "createdAt":   "2026-02-23T18:00:00Z"
    }
  }
}
```

**Errors:**
| Code | HTTP | When |
|------|------|------|
| `UNAUTHORIZED` | 401 | No / invalid token |
| `INVALID_UUID` | 400 | Malformed UUID |
| `INVALID_DATETIME` | 400 | `scheduled_at` wrong format |
| `PROVIDER_NOT_FOUND` | 404 | Provider doesn't exist |
| `SERVICE_NOT_FOUND` | 404 | Service doesn't exist |

---

### `PUT /api/v1/carecrew/bookings/{id}/status` 🔒

Update booking status.

**Auth:** ✅ Required

#### Request Body
```json
{ "status": "confirmed" }
```

**Allowed values:** `pending` `confirmed` `in_progress` `completed` `cancelled`

**Status Flow:**
```
pending ──► confirmed ──► in_progress ──► completed
  │              │               │
  ▼              ▼               ▼
cancelled    cancelled       cancelled
```
> `completed` and `cancelled` are terminal — cannot be changed further.

#### Response `200 OK`
```json
{
  "success": true,
  "message": "Booking status updated successfully",
  "data": { "booking": { "id": "uuid", "status": "confirmed" } }
}
```

**Errors:**
| Code | HTTP | When |
|------|------|------|
| `UNAUTHORIZED` | 401 | No / invalid token |
| `INVALID_STATUS` | 400 | Unknown status value |
| `INVALID_TRANSITION` | 422 | e.g. `pending → completed` (must go step by step) |
| `NOT_FOUND` | 404 | Booking not found |

---

### `GET /api/v1/carecrew/providers/{id}/bookings` 🔒

All bookings for a provider. Includes customer info.

**Auth:** ✅ Required

#### Query Params: `page` (default 1) | `limit` (default 10, max 50)

#### Response `200 OK`
```json
{
  "success": true,
  "message": "Bookings retrieved successfully",
  "data": {
    "bookings": [
      {
        "id":          "uuid",
        "providerId":  "uuid",
        "serviceId":   "uuid",
        "scheduledAt": "2026-03-15T10:00:00Z",
        "status":      "pending",
        "notes":       "Morning preferred",
        "createdAt":   "2026-02-23T18:00:00Z",
        "user": {
          "name":  "Ravi Sharma",
          "email": "ravi@example.com",
          "phone": "+91XXXXXXXXXX"
        }
      }
    ],
    "pagination": { "total_count": 8, "current_page": 1, "total_pages": 1, "limit": 10 }
  }
}
```

---

## All Endpoints

| Method | Endpoint | Auth | Description |
|--------|----------|------|-------------|
| GET | `/api/v1/properties/search` | ❌ | Search properties with filters & pagination |
| GET | `/api/v1/properties/filters` | ❌ | Dynamic filter options & ranges |
| GET | `/api/v1/search/suggestions?q=` | ❌ | Autocomplete suggestions |
| GET | `/api/v1/carecrew/services` | ❌ | List all services |
| GET | `/api/v1/carecrew/services/{id}` | ❌ | Service detail |
| GET | `/api/v1/carecrew/providers` | ❌ | Search providers |
| GET | `/api/v1/carecrew/providers/featured` | ❌ | Top featured providers |
| GET | `/api/v1/carecrew/providers/{id}` | ❌ | Provider detail |
| POST | `/api/v1/carecrew/bookings` | 🔒 | Create booking |
| PUT | `/api/v1/carecrew/bookings/{id}/status` | 🔒 | Update booking status |
| GET | `/api/v1/carecrew/providers/{id}/bookings` | 🔒 | Provider's bookings list |

---

## Error Codes

| Code | HTTP | Meaning |
|------|------|---------|
| `UNAUTHORIZED` | 401 | Missing or invalid Bearer token |
| `INVALID_TOKEN` | 401 | JWT expired or signature invalid |
| `INVALID_UUID` | 400 | UUID in path or body is malformed |
| `QUERY_TOO_SHORT` | 400 | Suggestions `q` param < 2 chars |
| `INVALID_DATETIME` | 400 | `scheduled_at` not ISO 8601 |
| `INVALID_STATUS` | 400 | Booking status not in allowed list |
| `INVALID_TRANSITION` | 422 | Booking state can't move to requested status |
| `NOT_FOUND` | 404 | Resource not found |
| `PROVIDER_NOT_FOUND` | 404 | Provider UUID inactive or missing |
| `SERVICE_NOT_FOUND` | 404 | Service UUID inactive or missing |
| `SEARCH_DB_ERROR` | 500 | DB error during property search |
| `FILTERS_DB_ERROR` | 500 | DB error during filter fetch |
| `SUGGESTIONS_DB_ERROR` | 500 | DB error during suggestions |
| `DB_ERROR` | 500 | General database error |
