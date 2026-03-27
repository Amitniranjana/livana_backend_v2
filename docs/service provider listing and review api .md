# Service Provider Listing + Review APIs
## For Frontend Team | Issue #43

**Base URLs**
| Environment | URL |
|---|---|
| Dev | http://localhost:8080 |
| Staging | https://api-staging.livanaeco.com |
| Production | https://api.livanaeco.com |

**Auth:** Every endpoint requires `Authorization: Bearer <jwt_token>` header  
**Prices:** All service prices are **HOURLY rates** in INR (₹)  
**Date Format:** ISO 8601 UTC — `2025-11-16T10:30:00Z`  
**Pagination:** All list endpoints accept `limit` (default 10, max 100) and `offset` (default 0)

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
| Code | HTTP | Meaning |
|---|---|---|
| `REVIEW_ALREADY_EXISTS` | 409 | Booking/visit already has a review |
| `BOOKING_NOT_COMPLETED` | 422 | Booking not yet completed |
| `VISIT_NOT_COMPLETED` | 422 | Visit not yet completed |
| `REVIEW_NOT_FOUND` | 404 | Review ID does not exist |
| `EDIT_PERIOD_EXPIRED` | 403 | 30-day edit window has passed |
| `REPLY_ALREADY_EXISTS` | 409 | Review already has a reply |
| `INVALID_RATING` | 400 | Rating outside 1.0–5.0 |
| `ACCESS_DENIED` | 403 | User does not own resource |
| `INVALID_TOKEN` | 401 | JWT missing / expired / invalid |
| `VALIDATION_ERROR` | 400 | Missing/invalid request fields |
| `NOT_FOUND` | 404 | Resource not found |

---

## 1. Service Provider APIs

### POST /api/services
Add a new service (provider only)

**Request Body:**
| Field | Type | Required | Notes |
|---|---|---|---|
| service_name | string | ✅ | |
| category | string | ✅ | `interior_designer` `packers_movers` `cleaning` `furniture_rental` `electrician` `plumber` |
| price | integer | ✅ | **Hourly rate** in INR |
| description | string | ✅ | |
| experience | string | ✅ | e.g. "8 years" |
| location | string | ✅ | |

**201 Response:**
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
    "description": "24/7 emergency plumbing",
    "experience": "8 years",
    "location": "Andheri West, Mumbai",
    "created_at": "2025-11-16T10:30:00Z"
  }
}
```

**cURL:**
```bash
curl -X POST https://api.livanaeco.com/api/services \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"service_name":"Emergency Plumbing","category":"plumber","price":500,"description":"24/7","experience":"8 years","location":"Mumbai"}'
```

**Possible errors:** `VALIDATION_ERROR`, `INVALID_TOKEN`

---

### GET /api/services
Get all services (paginated)

**Query Params:** `limit` `offset`

**200 Response:**
```json
{
  "success": true,
  "message": "Services retrieved successfully",
  "data": {
    "services": [{ "id": "uuid", "service_name": "...", "category": "plumber", "price": 500, "...": "..." }],
    "total_count": 156,
    "current_page": 1,
    "total_pages": 16
  }
}
```

**cURL:**
```bash
curl "https://api.livanaeco.com/api/services?limit=10&offset=0" \
  -H "Authorization: Bearer <token>"
```

---

### GET /api/services/providers
Filter providers by service type

**Query Params:**
| Param | Type | Required | Notes |
|---|---|---|---|
| service_type | string | ✅ | e.g. `plumber` |
| sort_by | string | No | `rating` \| `price` \| `experience` |
| limit | integer | No | default 10 |
| offset | integer | No | default 0 |
| latitude | float | No | |
| longitude | float | No | |

**200 Response:**
```json
{
  "success": true,
  "message": "Providers retrieved successfully",
  "data": {
    "providers": [{
      "id": "uuid", "name": "Rajesh Kumar", "service_type": "plumber",
      "rating": 4.8, "review_count": 234, "location": "Mumbai",
      "hourly_rate": 500.0, "experience": "8+ years",
      "is_verified": true, "availability": "available", "distance_km": null
    }],
    "total_count": 156, "current_page": 1, "total_pages": 8
  }
}
```

**cURL:**
```bash
curl "https://api.livanaeco.com/api/services/providers?service_type=plumber&sort_by=rating" \
  -H "Authorization: Bearer <token>"
```

**Possible errors:** `VALIDATION_ERROR` (missing service_type), `INVALID_TOKEN`

---

## 2. CareCrew Review APIs

### POST /api/reviews/carecrew
Submit a review (customer only, booking must be completed)

**Request Body:**
| Field | Type | Required |
|---|---|---|
| booking_id | UUID | ✅ |
| provider_id | UUID | ✅ |
| rating | float 1.0–5.0 | ✅ |
| comment | string | No |

**201 Response:**
```json
{
  "success": true,
  "message": "Review submitted successfully",
  "data": {
    "review_id": "uuid", "booking_id": "uuid", "provider_id": "uuid",
    "reviewer_id": "uuid", "rating": 4.5, "comment": "Great service",
    "created_at": "2025-11-16T10:30:00Z"
  }
}
```

**cURL:**
```bash
curl -X POST https://api.livanaeco.com/api/reviews/carecrew \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"booking_id":"uuid","provider_id":"uuid","rating":4.5,"comment":"Great!"}'
```

**Possible errors:** `BOOKING_NOT_COMPLETED`, `REVIEW_ALREADY_EXISTS`, `INVALID_RATING`, `INVALID_TOKEN`

---

### GET /api/reviews/carecrew/:provider_id
Get reviews for a provider with rating summary

**200 Response:**
```json
{
  "success": true,
  "message": "Reviews retrieved successfully",
  "data": {
    "reviews": [{
      "id": "uuid", "reviewer_name": "Priya Sharma", "reviewer_image": "url",
      "rating": 4.5, "comment": "...", "reply": "Thank you!", "reply_at": "...", "review_date": "..."
    }],
    "summary": {
      "average_rating": 4.5, "total_reviews": 234,
      "breakdown": {"5": 120, "4": 80, "3": 20, "2": 10, "1": 4}
    },
    "total_count": 234, "current_page": 1, "total_pages": 24
  }
}
```

---

### PUT /api/reviews/carecrew/:review_id
Edit own review (within 30 days)

**Request Body:** `rating` (optional float), `comment` (optional string)

**Possible errors:** `REVIEW_NOT_FOUND`, `ACCESS_DENIED`, `EDIT_PERIOD_EXPIRED`, `INVALID_RATING`

---

### DELETE /api/reviews/carecrew/:review_id
Delete own review (within 30 days)

**Possible errors:** `REVIEW_NOT_FOUND`, `ACCESS_DENIED`, `EDIT_PERIOD_EXPIRED`

---

### POST /api/reviews/carecrew/:review_id/reply
Provider replies to a review (one reply only)

**Request Body:** `{ "reply": "string" }`

**Possible errors:** `REVIEW_NOT_FOUND`, `ACCESS_DENIED`, `REPLY_ALREADY_EXISTS`

---

## 3. Property Review APIs

### POST /api/reviews/property
Submit a property review (visit must be completed)

**Request Body:**
| Field | Type | Required |
|---|---|---|
| visit_id | UUID | ✅ |
| property_id | UUID | ✅ |
| rating | float 1.0–5.0 | ✅ |
| comment | string | No |
| location_rating | float 1.0–5.0 | No |
| cleanliness_rating | float 1.0–5.0 | No |
| value_rating | float 1.0–5.0 | No |

**201 Response:**
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
    "comment": "Nice place",
    "location_rating": 4.5,
    "cleanliness_rating": 4.8,
    "value_rating": null,
    "created_at": "2025-11-16T10:30:00Z"
  }
}
```

**Possible errors:** `VISIT_NOT_COMPLETED`, `REVIEW_ALREADY_EXISTS`, `INVALID_RATING`

---

### GET /api/reviews/property/:property_id
Get reviews for a property with sub-rating averages

**200 Response:**
```json
{
  "success": true,
  "message": "Reviews retrieved successfully",
  "data": {
    "reviews": [{
      "id": "uuid", "reviewer_name": "Test Customer", "reviewer_image": null,
      "rating": 4.5, "comment": "Nice place", 
      "location_rating": 4.5, "cleanliness_rating": 4.8, "value_rating": null,
      "reply": null, "reply_at": null, "review_date": "2025-11-16T10:30:00Z"
    }],
    "summary": {
      "average_rating": 4.5, "total_reviews": 89,
      "average_location_rating": 4.2, "average_cleanliness_rating": 4.7, "average_value_rating": 4.3,
      "breakdown": {"5": 40, "4": 30, "3": 10, "2": 6, "1": 3}
    },
    "total_count": 89, "current_page": 1, "total_pages": 9
  }
}
```

---

### PUT /api/reviews/property/:review_id
Edit own property review (within 30 days, sub-ratings also updatable)

---

### DELETE /api/reviews/property/:review_id
Delete own property review (within 30 days)

---

### POST /api/reviews/property/:review_id/reply
Property owner replies (one reply only)

---

## Rate Limits
| Type | Limit |
|---|---|
| Authenticated requests | 100 req/min per user |
| File uploads | 10/hour per user |
