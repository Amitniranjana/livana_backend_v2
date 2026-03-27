# Care Connect App — API Reference for Frontend Team

**Base URLs:**
- Dev:     `http://localhost:8080`
- Staging: `https://api-staging.livanaeco.com`
- Prod:    `https://api.livanaeco.com`

**Auth:** All endpoints require:
```
Authorization: Bearer <jwt_token>
```

**Prices:** All service prices are **HOURLY rates in INR (₹)**
**Date Format:** ISO 8601 UTC — e.g. `2025-11-16T10:30:00Z`
**Currency:** INR (Indian Rupees)

---

## Pagination

All list endpoints accept:
- `limit` — items per page (default 10, max 100)
- `offset` — items to skip (default 0)

Response always includes:
```json
"total_count": 156,
"current_page": 1,
"total_pages": 16
```

---

## Error Format

```json
{
  "success": false,
  "message": "Human readable error",
  "error_code": "ERROR_CODE",
  "errors": null
}
```

### Error Code Reference

| Code | HTTP Status | When |
|---|---|---|
| `VALIDATION_ERROR` | 400 | Missing/invalid fields |
| `INVALID_RATING` | 400 | Rating outside 1.0–5.0 |
| `INVALID_TOKEN` | 401 | JWT missing, expired, or invalid |
| `ACCESS_DENIED` | 403 | User does not own the resource |
| `EDIT_PERIOD_EXPIRED` | 403 | >30 days since review creation |
| `NOT_FOUND` | 404 | Resource not found |
| `REVIEW_NOT_FOUND` | 404 | review_id does not exist |
| `REVIEW_ALREADY_EXISTS` | 409 | Booking/visit already reviewed |
| `REPLY_ALREADY_EXISTS` | 409 | Review already has a reply |
| `BOOKING_NOT_COMPLETED` | 422 | Booking status ≠ completed |
| `VISIT_NOT_COMPLETED` | 422 | Visit status ≠ completed |

---

## Rate Limits
- Authenticated: 100 requests/minute per user
- File upload: 10 uploads/hour per user

---

## 1. SERVICE PROVIDER APIS

---

### POST /api/services
**Description:** Add a new service listing
**Auth:** Required

**Request Body:**
| Field | Type | Required | Notes |
|---|---|---|---|
| service_name | String | ✅ | |
| category | String | ✅ | One of: `interior_designer`, `packers_movers`, `cleaning`, `furniture_rental`, `electrician`, `plumber` |
| price | Integer | ✅ | Hourly rate in INR |
| description | String | ✅ | |
| experience | String | ✅ | e.g. "8 years" |
| location | String | ✅ | |

**Success Response — `201 Created`:**
```json
{
  "success": true,
  "message": "Service added successfully",
  "data": {
    "service_id": "uuid",
    "provider_id": "uuid",
    "service_name": "Emergency Plumbing",
    "category": "plumber",
    "price": 500,
    "description": "24/7 emergency plumbing services",
    "experience": "8 years",
    "location": "Andheri West, Mumbai",
    "created_at": "2025-11-16T10:30:00Z"
  }
}
```

**Error Codes:** `VALIDATION_ERROR`, `INVALID_TOKEN`

**cURL Example:**
```bash
curl -X POST https://api.livanaeco.com/api/services \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "service_name": "Emergency Plumbing",
    "category": "plumber",
    "price": 500,
    "description": "24/7 emergency plumbing services",
    "experience": "8 years",
    "location": "Andheri West, Mumbai"
  }'
```

---

### GET /api/services
**Description:** Get all services (paginated)
**Auth:** Required

**Query Parameters:**
| Param | Type | Default | Required |
|---|---|---|---|
| limit | Integer | 10 | No (max 100) |
| offset | Integer | 0 | No |

**Success Response — `200 OK`:**
```json
{
  "success": true,
  "message": "Services retrieved successfully",
  "data": {
    "services": [
      {
        "id": "uuid",
        "provider_id": "uuid",
        "service_name": "Emergency Plumbing",
        "category": "plumber",
        "price": 500,
        "description": "24/7 emergency plumbing services",
        "experience": "8 years",
        "location": "Andheri West, Mumbai",
        "created_at": "2025-11-16T10:30:00Z"
      }
    ],
    "total_count": 156,
    "current_page": 1,
    "total_pages": 16
  }
}
```

**Error Codes:** `INVALID_TOKEN`

**cURL Example:**
```bash
curl -X GET "https://api.livanaeco.com/api/services?limit=10&offset=0" \
  -H "Authorization: Bearer <token>"
```

---

### GET /api/services/providers
**Description:** Filter providers by service type
**Auth:** Required

**Query Parameters:**
| Param | Type | Required | Notes |
|---|---|---|---|
| service_type | String | ✅ | e.g. `"plumber"` |
| sort_by | String | No | `"rating"` \| `"price"` \| `"experience"` (default: `"rating"`) |
| limit | Integer | No | Default 10 |
| offset | Integer | No | Default 0 |
| latitude | Float | No | For distance calculation |
| longitude | Float | No | For distance calculation |

**Success Response — `200 OK`:**
```json
{
  "success": true,
  "message": "Providers retrieved successfully",
  "data": {
    "providers": [
      {
        "id": "uuid",
        "name": "Rajesh Kumar",
        "service_type": "plumber",
        "rating": 4.8,
        "review_count": 234,
        "location": "Andheri West, Mumbai",
        "hourly_rate": 500.0,
        "experience": "8+ years",
        "is_verified": true,
        "availability": "available",
        "distance_km": null
      }
    ],
    "total_count": 156,
    "current_page": 1,
    "total_pages": 8
  }
}
```

**Error Codes:** `VALIDATION_ERROR` (missing service_type), `INVALID_TOKEN`

**cURL Example:**
```bash
curl -X GET "https://api.livanaeco.com/api/services/providers?service_type=plumber&sort_by=rating&limit=10" \
  -H "Authorization: Bearer <token>"
```

---

## 2. CARECREW REVIEW APIS

---

### POST /api/reviews/carecrew
**Description:** Submit a review for a CareCrew provider
**Auth:** Required (reviewer_id from JWT)

**Request Body:**
| Field | Type | Required | Notes |
|---|---|---|---|
| booking_id | UUID | ✅ | Must reference a completed booking |
| provider_id | UUID | ✅ | |
| rating | Float | ✅ | 1.0 – 5.0 |
| comment | String | No | |

**Success Response — `201 Created`:**
```json
{
  "success": true,
  "message": "Review submitted successfully",
  "data": {
    "review_id": "uuid",
    "booking_id": "uuid",
    "provider_id": "uuid",
    "reviewer_id": "uuid",
    "rating": 4.5,
    "comment": "Very professional and on time.",
    "created_at": "2025-11-16T10:30:00Z"
  }
}
```

**Error Codes:** `BOOKING_NOT_COMPLETED` (422), `REVIEW_ALREADY_EXISTS` (409), `INVALID_RATING` (400), `INVALID_TOKEN` (401)

**cURL Example:**
```bash
curl -X POST https://api.livanaeco.com/api/reviews/carecrew \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "booking_id": "550e8400-e29b-41d4-a716-446655440000",
    "provider_id": "550e8400-e29b-41d4-a716-446655440001",
    "rating": 4.5,
    "comment": "Very professional and on time."
  }'
```

---

### GET /api/reviews/carecrew/{provider_id}
**Description:** Get reviews for a provider with rating summary
**Auth:** Required

**Path Parameter:** `provider_id` (UUID)

**Query Parameters:** `limit` (default 10), `offset` (default 0)

**Success Response — `200 OK`:**
```json
{
  "success": true,
  "message": "Reviews retrieved successfully",
  "data": {
    "reviews": [
      {
        "id": "uuid",
        "reviewer_name": "Priya Sharma",
        "reviewer_image": "https://cdn.example.com/image.jpg",
        "rating": 4.5,
        "comment": "Very professional.",
        "reply": "Thank you!",
        "reply_at": "2025-11-17T08:00:00Z",
        "review_date": "2025-11-16T10:30:00Z"
      }
    ],
    "summary": {
      "average_rating": 4.5,
      "total_reviews": 234,
      "breakdown": { "5": 120, "4": 80, "3": 20, "2": 10, "1": 4 }
    },
    "total_count": 234,
    "current_page": 1,
    "total_pages": 24
  }
}
```

**Error Codes:** `NOT_FOUND` (404), `INVALID_TOKEN` (401)

**cURL Example:**
```bash
curl -X GET "https://api.livanaeco.com/api/reviews/carecrew/550e8400-e29b-41d4-a716-446655440001?limit=10&offset=0" \
  -H "Authorization: Bearer <token>"
```

---

### PUT /api/reviews/carecrew/{review_id}
**Description:** Edit a CareCrew review (within 30 days, own review only)
**Auth:** Required

**Path Parameter:** `review_id` (UUID)

**Request Body:**
| Field | Type | Required | Notes |
|---|---|---|---|
| rating | Float | No | 1.0 – 5.0 |
| comment | String | No | |

**Success Response — `200 OK`:**
```json
{
  "success": true,
  "message": "Review updated successfully",
  "data": {
    "review_id": "uuid",
    "rating": 4.0,
    "comment": "Updated: service was good but a bit late.",
    "updated_at": "2025-11-17T10:00:00Z"
  }
}
```

**Error Codes:** `REVIEW_NOT_FOUND` (404), `ACCESS_DENIED` (403), `EDIT_PERIOD_EXPIRED` (403), `INVALID_RATING` (400)

**cURL Example:**
```bash
curl -X PUT https://api.livanaeco.com/api/reviews/carecrew/550e8400-e29b-41d4-a716-446655440002 \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{ "rating": 4.0, "comment": "Updated comment." }'
```

---

### DELETE /api/reviews/carecrew/{review_id}
**Description:** Delete a CareCrew review (within 30 days, own review only)
**Auth:** Required

**Path Parameter:** `review_id` (UUID)

**Success Response — `200 OK`:**
```json
{
  "success": true,
  "message": "Review deleted successfully",
  "data": { "deleted": true, "review_id": "uuid" }
}
```

**Error Codes:** `REVIEW_NOT_FOUND` (404), `ACCESS_DENIED` (403), `EDIT_PERIOD_EXPIRED` (403)

**cURL Example:**
```bash
curl -X DELETE https://api.livanaeco.com/api/reviews/carecrew/550e8400-e29b-41d4-a716-446655440002 \
  -H "Authorization: Bearer <token>"
```

---

### POST /api/reviews/carecrew/{review_id}/reply
**Description:** Provider replies to a review (one reply per review)
**Auth:** Required (must be the reviewed provider)

**Path Parameter:** `review_id` (UUID)

**Request Body:**
| Field | Type | Required |
|---|---|---|
| reply | String | ✅ |

**Success Response — `201 Created`:**
```json
{
  "success": true,
  "message": "Reply added successfully",
  "data": {
    "review_id": "uuid",
    "reply": "Thank you for your feedback!",
    "replied_at": "2025-11-17T08:00:00Z"
  }
}
```

**Error Codes:** `REVIEW_NOT_FOUND` (404), `ACCESS_DENIED` (403), `REPLY_ALREADY_EXISTS` (409)

**cURL Example:**
```bash
curl -X POST https://api.livanaeco.com/api/reviews/carecrew/550e8400-e29b-41d4-a716-446655440002/reply \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{ "reply": "Thank you for your feedback!" }'
```

---

## 3. PROPERTY REVIEW APIS

---

### POST /api/reviews/property
**Description:** Submit a review for a property
**Auth:** Required (reviewer_id from JWT)

**Request Body:**
| Field | Type | Required | Notes |
|---|---|---|---|
| visit_id | UUID | ✅ | Must reference a completed visit |
| property_id | UUID | ✅ | |
| rating | Float | ✅ | 1.0 – 5.0 |
| comment | String | No | |
| location_rating | Float | No | 1.0 – 5.0 if provided |
| cleanliness_rating | Float | No | 1.0 – 5.0 if provided |
| value_rating | Float | No | 1.0 – 5.0 if provided |

**Success Response — `201 Created`:**
```json
{
  "success": true,
  "message": "Review submitted successfully",
  "data": {
    "review_id": "uuid",
    "visit_id": "uuid",
    "property_id": "uuid",
    "reviewer_id": "uuid",
    "rating": 4.5,
    "comment": "Great location, well maintained.",
    "location_rating": 4.0,
    "cleanliness_rating": 5.0,
    "value_rating": 4.5,
    "created_at": "2025-11-16T10:30:00Z"
  }
}
```

**Error Codes:** `VISIT_NOT_COMPLETED` (422), `REVIEW_ALREADY_EXISTS` (409), `INVALID_RATING` (400), `INVALID_TOKEN` (401)

**cURL Example:**
```bash
curl -X POST https://api.livanaeco.com/api/reviews/property \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "visit_id": "550e8400-e29b-41d4-a716-446655440003",
    "property_id": "550e8400-e29b-41d4-a716-446655440004",
    "rating": 4.5,
    "comment": "Great location, well maintained.",
    "location_rating": 4.0,
    "cleanliness_rating": 5.0,
    "value_rating": 4.5
  }'
```

---

### GET /api/reviews/property/{property_id}
**Description:** Get reviews for a property with rating summary (including sub-ratings)
**Auth:** Required

**Path Parameter:** `property_id` (UUID)

**Query Parameters:** `limit` (default 10), `offset` (default 0)

**Success Response — `200 OK`:**
```json
{
  "success": true,
  "message": "Reviews retrieved successfully",
  "data": {
    "reviews": [
      {
        "id": "uuid",
        "reviewer_name": "Amit Singh",
        "reviewer_image": "string",
        "rating": 4.5,
        "comment": "Great location, well maintained.",
        "location_rating": 4.0,
        "cleanliness_rating": 5.0,
        "value_rating": 4.5,
        "reply": null,
        "review_date": "2025-11-16T10:30:00Z"
      }
    ],
    "summary": {
      "average_rating": 4.5,
      "total_reviews": 89,
      "average_location_rating": 4.2,
      "average_cleanliness_rating": 4.7,
      "average_value_rating": 4.3,
      "breakdown": { "5": 40, "4": 30, "3": 10, "2": 6, "1": 3 }
    },
    "total_count": 89,
    "current_page": 1,
    "total_pages": 9
  }
}
```

**Error Codes:** `INVALID_TOKEN` (401)

**cURL Example:**
```bash
curl -X GET "https://api.livanaeco.com/api/reviews/property/550e8400-e29b-41d4-a716-446655440004?limit=10&offset=0" \
  -H "Authorization: Bearer <token>"
```

---

### PUT /api/reviews/property/{review_id}
**Description:** Edit a property review (within 30 days, own review only)
**Auth:** Required

**Path Parameter:** `review_id` (UUID)

**Request Body:**
| Field | Type | Required | Notes |
|---|---|---|---|
| rating | Float | No | 1.0 – 5.0 |
| comment | String | No | |
| location_rating | Float | No | 1.0 – 5.0 |
| cleanliness_rating | Float | No | 1.0 – 5.0 |
| value_rating | Float | No | 1.0 – 5.0 |

**Success Response — `200 OK`:**
```json
{
  "success": true,
  "message": "Review updated successfully",
  "data": {
    "review_id": "uuid",
    "rating": 4.0,
    "comment": "Updated comment.",
    "location_rating": 3.5,
    "cleanliness_rating": 4.5,
    "value_rating": 4.0,
    "updated_at": "2025-11-17T10:00:00Z"
  }
}
```

**Error Codes:** `REVIEW_NOT_FOUND` (404), `ACCESS_DENIED` (403), `EDIT_PERIOD_EXPIRED` (403), `INVALID_RATING` (400)

**cURL Example:**
```bash
curl -X PUT https://api.livanaeco.com/api/reviews/property/550e8400-e29b-41d4-a716-446655440005 \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{ "rating": 4.0, "comment": "Updated.", "location_rating": 3.5 }'
```

---

### DELETE /api/reviews/property/{review_id}
**Description:** Delete a property review (within 30 days, own review only)
**Auth:** Required

**Path Parameter:** `review_id` (UUID)

**Success Response — `200 OK`:**
```json
{
  "success": true,
  "message": "Review deleted successfully",
  "data": { "deleted": true, "review_id": "uuid" }
}
```

**Error Codes:** `REVIEW_NOT_FOUND` (404), `ACCESS_DENIED` (403), `EDIT_PERIOD_EXPIRED` (403)

**cURL Example:**
```bash
curl -X DELETE https://api.livanaeco.com/api/reviews/property/550e8400-e29b-41d4-a716-446655440005 \
  -H "Authorization: Bearer <token>"
```

---

### POST /api/reviews/property/{review_id}/reply
**Description:** Property owner replies to a review (one reply per review)
**Auth:** Required (must be the property owner)

**Path Parameter:** `review_id` (UUID)

**Request Body:**
| Field | Type | Required |
|---|---|---|
| reply | String | ✅ |

**Success Response — `201 Created`:**
```json
{
  "success": true,
  "message": "Reply added successfully",
  "data": {
    "review_id": "uuid",
    "reply": "Thank you for staying with us!",
    "replied_at": "2025-11-17T08:00:00Z"
  }
}
```

**Error Codes:** `REVIEW_NOT_FOUND` (404), `ACCESS_DENIED` (403), `REPLY_ALREADY_EXISTS` (409)

**cURL Example:**
```bash
curl -X POST https://api.livanaeco.com/api/reviews/property/550e8400-e29b-41d4-a716-446655440005/reply \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{ "reply": "Thank you for staying with us!" }'
```
