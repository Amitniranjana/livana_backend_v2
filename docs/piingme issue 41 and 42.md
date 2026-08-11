# Piingme — Frontend API Documentation
## Issues 40 & 41: Create Ping + List My Pings

> **Base URL:** `http://<server>:9090`  
> **Auth:** All endpoints below require a valid JWT Bearer token in the `Authorization` header.  
> **Content-Type:** `application/json`

---

## Issue 40 — POST `/api/v1/pings`

**Summary:** Create a new property requirement post (a "Ping"). On success, the backend automatically matches verified brokers in the same location and fans out in-app notifications to them — the frontend does **not** need to make any extra call.

### Request

```
POST /api/v1/pings
Authorization: Bearer <JWT>
Content-Type: application/json
```

#### Request Body

| Field           | Type     | Required | Description                                              |
|-----------------|----------|----------|----------------------------------------------------------|
| `location`      | `string` | ✅ Yes   | Human-readable location name (e.g. `"Bandra West"`)     |
| `latitude`      | `number` | ❌ No    | Decimal latitude (e.g. `19.0596`)                        |
| `longitude`     | `number` | ❌ No    | Decimal longitude (e.g. `72.8295`)                       |
| `property_type` | `string` | ❌ No    | e.g. `"Apartment"`, `"Villa"`, `"Plot"`                  |
| `listing_type`  | `string` | ❌ No    | e.g. `"buy"`, `"rent"`, `"lease"`                        |
| `min_budget`    | `number` | ❌ No    | Minimum budget in INR (integer)                          |
| `max_budget`    | `number` | ❌ No    | Maximum budget in INR (integer)                          |
| `min_bedrooms`  | `number` | ❌ No    | Minimum number of bedrooms (integer)                     |
| `max_bedrooms`  | `number` | ❌ No    | Maximum number of bedrooms (integer)                     |
| `note`          | `string` | ❌ No    | Free-text note / additional requirements                 |

#### Example Request Body

```json
{
  "location": "Bandra West",
  "latitude": 19.0596,
  "longitude": 72.8295,
  "property_type": "Apartment",
  "listing_type": "buy",
  "min_budget": 5000000,
  "max_budget": 10000000,
  "min_bedrooms": 2,
  "max_bedrooms": 3,
  "note": "Prefer a south-facing flat with parking."
}
```

### Response

#### `201 Created` — Success

```json
{
  "success": true,
  "message": "Ping created successfully",
  "data": {
    "id": "3fa85f64-5717-4562-b3fc-2c963f66afa6",
    "user_id": "a1b2c3d4-...",
    "location": "Bandra West",
    "latitude": 19.0596,
    "longitude": 72.8295,
    "property_type": "Apartment",
    "listing_type": "buy",
    "min_budget": 5000000,
    "max_budget": 10000000,
    "min_bedrooms": 2,
    "max_bedrooms": 3,
    "note": "Prefer a south-facing flat with parking.",
    "status": "active",
    "close_reason": null,
    "created_at": "2026-08-09T11:00:00Z",
    "updated_at": "2026-08-09T11:00:00Z"
  }
}
```

#### Error Responses

| HTTP Code | Meaning                                        |
|-----------|------------------------------------------------|
| `401`     | Missing or invalid JWT token                   |
| `500`     | Database error                                 |

### Backend Side-Effect (Issue 47 / Issue 49)

After the ping is created, the backend:
1. Queries `broker_profiles` for VERIFIED brokers whose `operating_cities` include the provided `location`.
2. Inserts an in-app notification for each matched broker with type `PING`, so they see the request in their feed.

**The Flutter app does not need to trigger this separately.** It is atomic with the create call.

---

## Issue 41 — GET `/api/v1/pings/mine`

**Summary:** Fetch the currently logged-in user's own pings. Supports filtering by status.

### Request

```
GET /api/v1/pings/mine?status=active
Authorization: Bearer <JWT>
```

#### Query Parameters

| Parameter | Type     | Required | Allowed Values              | Default (if omitted)                                       |
|-----------|----------|----------|-----------------------------|------------------------------------------------------------|
| `status`  | `string` | ❌ No    | `active`, `closed`, `all`   | Returns all non-deleted pings (both active and closed)     |

> **Note:** Deleted pings are never returned regardless of the `status` filter. Use `status=all` to fetch both active and closed pings together.

#### Example Requests

```
GET /api/v1/pings/mine                  → all non-deleted pings
GET /api/v1/pings/mine?status=active    → only active pings
GET /api/v1/pings/mine?status=closed    → only closed pings
GET /api/v1/pings/mine?status=all       → active + closed (same as no filter)
```

### Response

#### `200 OK` — Success

```json
{
  "success": true,
  "message": "Pings fetched successfully",
  "data": [
    {
      "id": "3fa85f64-5717-4562-b3fc-2c963f66afa6",
      "user_id": "a1b2c3d4-...",
      "location": "Bandra West",
      "latitude": 19.0596,
      "longitude": 72.8295,
      "property_type": "Apartment",
      "listing_type": "buy",
      "min_budget": 5000000,
      "max_budget": 10000000,
      "min_bedrooms": 2,
      "max_bedrooms": 3,
      "note": "Prefer a south-facing flat with parking.",
      "status": "active",
      "close_reason": null,
      "created_at": "2026-08-09T11:00:00Z",
      "updated_at": "2026-08-09T11:00:00Z"
    }
  ]
}
```

> Returns an **empty array `[]`** if the user has no pings matching the filter — never returns `null`.

#### Response Object — `PingDto` Fields

| Field           | Type                    | Nullable | Description                                       |
|-----------------|-------------------------|----------|---------------------------------------------------|
| `id`            | `string (UUID)`         | No       | Unique ping ID                                    |
| `user_id`       | `string (UUID)`         | No       | Owner's user ID                                   |
| `location`      | `string`                | No       | Location text                                     |
| `latitude`      | `number`                | Yes      | GPS latitude                                      |
| `longitude`     | `number`                | Yes      | GPS longitude                                     |
| `property_type` | `string`                | Yes      | Type of property                                  |
| `listing_type`  | `string`                | Yes      | `buy` / `rent` / `lease`                          |
| `min_budget`    | `number`                | Yes      | Minimum budget (INR)                              |
| `max_budget`    | `number`                | Yes      | Maximum budget (INR)                              |
| `min_bedrooms`  | `number`                | Yes      | Minimum bedrooms                                  |
| `max_bedrooms`  | `number`                | Yes      | Maximum bedrooms                                  |
| `note`          | `string`                | Yes      | Additional note                                   |
| `status`        | `string`                | No       | `active` or `closed`                              |
| `close_reason`  | `string`                | Yes      | Reason (only set when `status = "closed"`)        |
| `created_at`    | `string (ISO 8601 UTC)` | No       | Creation timestamp                                |
| `updated_at`    | `string (ISO 8601 UTC)` | No       | Last updated timestamp                            |

#### Error Responses

| HTTP Code | Meaning                   |
|-----------|---------------------------|
| `401`     | Missing or invalid JWT    |
| `500`     | Database error            |

---

## Related Endpoints (For Reference)

These are also live and ready to integrate:

| Method   | Endpoint                            | Who Can Call     | Purpose                                              |
|----------|-------------------------------------|------------------|------------------------------------------------------|
| `GET`    | `/api/v1/pings/{pingId}`            | Any auth user    | Get full detail of a single ping                     |
| `PATCH`  | `/api/v1/pings/{pingId}/close`      | Ping owner only  | Close an active ping (body: `{ "reason": "..." }`)   |
| `DELETE` | `/api/v1/pings/{pingId}`            | Ping owner only  | Soft-delete a ping                                   |
| `GET`    | `/api/v1/pings/matching`            | Brokers          | Browse active pings (filterable by location/type)    |
| `POST`   | `/api/v1/pings/{pingId}/respond`    | Brokers          | Respond to a ping (auto-creates a chat thread)       |
| `GET`    | `/api/v1/pings/{pingId}/responses`  | Ping owner only  | List broker responses on a ping                      |

---

## Flutter Integration Tips

### Dart Model for `PingDto`

```dart
class PingDto {
  final String id;
  final String userId;
  final String location;
  final double? latitude;
  final double? longitude;
  final String? propertyType;
  final String? listingType;
  final int? minBudget;
  final int? maxBudget;
  final int? minBedrooms;
  final int? maxBedrooms;
  final String? note;
  final String status; // "active" | "closed"
  final String? closeReason;
  final DateTime createdAt;
  final DateTime updatedAt;

  PingDto({
    required this.id,
    required this.userId,
    required this.location,
    this.latitude,
    this.longitude,
    this.propertyType,
    this.listingType,
    this.minBudget,
    this.maxBudget,
    this.minBedrooms,
    this.maxBedrooms,
    this.note,
    required this.status,
    this.closeReason,
    required this.createdAt,
    required this.updatedAt,
  });

  factory PingDto.fromJson(Map<String, dynamic> json) => PingDto(
    id: json['id'],
    userId: json['user_id'],
    location: json['location'],
    latitude: (json['latitude'] as num?)?.toDouble(),
    longitude: (json['longitude'] as num?)?.toDouble(),
    propertyType: json['property_type'],
    listingType: json['listing_type'],
    minBudget: json['min_budget'],
    maxBudget: json['max_budget'],
    minBedrooms: json['min_bedrooms'],
    maxBedrooms: json['max_bedrooms'],
    note: json['note'],
    status: json['status'],
    closeReason: json['close_reason'],
    createdAt: DateTime.parse(json['created_at']),
    updatedAt: DateTime.parse(json['updated_at']),
  );
}
```

### Dio Example — Create Ping (Issue 40)

```dart
final response = await dio.post(
  '/api/v1/pings',
  data: {
    'location': 'Bandra West',
    'latitude': 19.0596,
    'longitude': 72.8295,
    'property_type': 'Apartment',
    'listing_type': 'buy',
    'min_budget': 5000000,
    'max_budget': 10000000,
    'min_bedrooms': 2,
    'max_bedrooms': 3,
    'note': 'Prefer south-facing with parking.',
  },
);
final ping = PingDto.fromJson(response.data['data']);
```

### Dio Example — My Pings (Issue 41)

```dart
final response = await dio.get(
  '/api/v1/pings/mine',
  queryParameters: {'status': 'active'}, // omit for all
);
final pings = (response.data['data'] as List)
    .map((e) => PingDto.fromJson(e))
    .toList();
```

---

*Documentation generated: 2026-08-09 | Backend: Livana Backend V2 (Rust/Axum 0.8.6)*
