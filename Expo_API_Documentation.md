# Expo Event API Documentation

## 1. Create Expo Event

Creates a new property expo event. This endpoint is used by Admins/Builders to create new expo events.

- **Endpoint:** `POST /api/expo`
- **Requires Authentication:** Yes (Bearer Token)

### Request Body

```json
{
  "title": "Ahmedabad Property Expo 2026",
  "description": "Real estate expo for builders, brokers and buyers",
  "location": "Ahmedabad Convention Center",
  "event_date": "2026-05-10",
  "start_time": "10:00",
  "end_time": "18:00",
  "organizer_id": "uuid",
  "banner_image": "file_url",
  "max_participants": 500
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| title | String | Yes | Name of the expo event |
| description | String | Yes | Detailed description of the event |
| location | String | Yes | Venue/address of the event |
| event_date | String | Yes | Date in `YYYY-MM-DD` format |
| start_time | String | Yes | Start time in `HH:MM` format |
| end_time | String | Yes | End time in `HH:MM` format |
| organizer_id | String (UUID) | Yes | UUID of the organizer |
| banner_image | String | No | URL/path of the banner image |
| max_participants | Integer | Yes | Maximum allowed participants (must be > 0) |

### Responses

**Success (201 Created)**
```json
{
  "success": true,
  "message": "Expo event created successfully",
  "data": {
    "expo_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    "title": "Ahmedabad Property Expo 2026",
    "created_at": "2026-05-01T10:30:00+00:00"
  }
}
```

**Client Errors**

- **400 Bad Request** (Empty/invalid fields)
```json
{
  "success": false,
  "message": "Title cannot be empty",
  "error_code": "BAD_REQUEST"
}
```

- **401 Unauthorized** (Missing/invalid JWT)
```json
{
  "success": false,
  "message": "Missing or invalid authorization header",
  "data": null
}
```

---

## 2. Get All Expo Events

Retrieves a paginated list of all expo events. Supports optional city-based filtering.

- **Endpoint:** `GET /api/expo`
- **Requires Authentication:** Yes (Bearer Token)

### Query Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| limit | Integer | 10 | Number of items per page (max: 100) |
| offset | Integer | 0 | Number of items to skip |
| city | String | — | Optional filter by location (case-insensitive partial match) |

### Example Requests
```
GET /api/expo
GET /api/expo?limit=5&offset=0
GET /api/expo?city=Ahmedabad&limit=10
```

### Responses

**Success (200 OK)**
```json
{
  "success": true,
  "message": "Expo events fetched successfully",
  "data": {
    "events": [
      {
        "expo_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
        "title": "Ahmedabad Property Expo",
        "location": "Ahmedabad Convention Center",
        "event_date": "2026-05-10",
        "registered_count": 120
      },
      {
        "expo_id": "b2c3d4e5-f6a7-8901-bcde-f12345678901",
        "title": "Mumbai Real Estate Summit",
        "location": "Mumbai Exhibition Center",
        "event_date": "2026-06-15",
        "registered_count": 85
      }
    ]
  }
}
```

**Empty result (200 OK)**
```json
{
  "success": true,
  "message": "Expo events fetched successfully",
  "data": {
    "events": []
  }
}
```

- **401 Unauthorized** (Missing/invalid JWT)
```json
{
  "success": false,
  "message": "Missing or invalid authorization header",
  "data": null
}
```

---

## 3. Expo Event Details

Retrieves full details of a specific expo event including live participant count and available services.

- **Endpoint:** `GET /api/expo/{expo_id}`
- **Requires Authentication:** Yes (Bearer Token)

### Path Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| expo_id | UUID | ID of the expo event |

### Request
No request body or query parameters are required.

### Responses

**Success (200 OK)**
```json
{
  "success": true,
  "message": "Expo event details retrieved successfully",
  "data": {
    "expo_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    "title": "Ahmedabad Property Expo 2026",
    "description": "Real estate expo for builders, brokers and buyers",
    "location": "Ahmedabad Convention Center",
    "event_date": "2026-05-10",
    "start_time": "10:00",
    "end_time": "18:00",
    "organizer_id": "f1e2d3c4-b5a6-7890-abcd-ef1234567890",
    "banner_image": "https://example.com/banner.jpg",
    "max_participants": 500,
    "participants_count": 120,
    "services_available": ["Interior Design", "Plumber", "Electrician"],
    "created_at": "2026-05-01T10:30:00+00:00"
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| expo_id | UUID | Unique identifier of the expo event |
| title | String | Name of the expo event |
| description | String | Detailed description |
| location | String | Venue/address |
| event_date | String | Date in `YYYY-MM-DD` format |
| start_time | String | Start time in `HH:MM` format |
| end_time | String | End time in `HH:MM` format |
| organizer_id | UUID | UUID of the organizer |
| banner_image | String | URL/path of the banner image |
| max_participants | Integer | Maximum allowed participants |
| participants_count | Integer | Current number of registered participants (live count) |
| services_available | Array\<String\> | List of services available at the expo |
| created_at | String | ISO 8601 timestamp of creation |

**Client Errors**

- **404 Not Found** (Invalid expo_id)
```json
{
  "success": false,
  "message": "Expo event not found",
  "error_code": "NOT_FOUND"
}
```

- **401 Unauthorized** (Missing/invalid JWT)
```json
{
  "success": false,
  "message": "Missing or invalid authorization header",
  "data": null
}
```

---

## 4. Register for Expo

Registers a user for a specific expo event. Validates capacity and prevents duplicate registrations.

- **Endpoint:** `POST /api/expo/{expo_id}/register`
- **Requires Authentication:** Yes (Bearer Token)

### Path Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| expo_id | UUID | ID of the expo event to register for |

### Request Body

```json
{
  "user_id": "c3d4e5f6-a7b8-9012-cdef-123456789012",
  "user_type": "broker",
  "company_name": "ABC Realty"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| user_id | String (UUID) | Yes | UUID of the user registering |
| user_type | String | Yes | Type of user — `"broker"`, `"landlord"`, `"service_provider"`, or `"user"` |
| company_name | String | No | Name of the user's company (optional) |

### Responses

**Success (201 Created)**
```json
{
  "success": true,
  "message": "Registered successfully for expo"
}
```

**Client Errors**

- **400 Bad Request** (Invalid user_id or empty user_type)
```json
{
  "success": false,
  "message": "Invalid user_id UUID",
  "error_code": "BAD_REQUEST"
}
```

- **404 Not Found** (Invalid expo_id)
```json
{
  "success": false,
  "message": "Expo event not found",
  "error_code": "NOT_FOUND"
}
```

- **409 Conflict** (Expo is full)
```json
{
  "success": false,
  "message": "Expo event is full. Maximum participants reached",
  "error_code": "CONFLICT"
}
```

- **409 Conflict** (Already registered)
```json
{
  "success": false,
  "message": "You are already registered for this expo event",
  "error_code": "CONFLICT"
}
```

- **401 Unauthorized** (Missing/invalid JWT)
```json
{
  "success": false,
  "message": "Missing or invalid authorization header",
  "data": null
}
```
