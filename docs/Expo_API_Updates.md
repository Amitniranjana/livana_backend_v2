# 📢 Expo Event System - API Updates

Hey Frontend Team! 

We've deployed some new features to the Expo Event System, including a new endpoint for fetching your own expos, support for geo-coordinates (`lat` / `lng`), and an automated push notification system for nearby users.

Here is the updated API documentation:

## 1. Get My Expos (NEW)
Fetches a paginated list of all expo events created by the currently authenticated user (Organizer).

**Endpoint:** `GET /api/expo/mine`
**Headers:** `Authorization: Bearer <JWT_TOKEN>`

**Query Parameters:**
- `limit` (optional): Default 10. Maximum 100.
- `offset` (optional): Default 0.

**Response (200 OK):**
```json
{
  "success": true,
  "message": "My expo events fetched successfully",
  "data": {
    "events": [
      {
        "expo_id": "550e8400-e29b-41d4-a716-446655440000",
        "title": "Luxury Real Estate Expo 2026",
        "location": "HITEC City, Hyderabad",
        "event_date": "2026-05-10",
        "registered_count": 45,
        "lat": 17.4474,
        "lng": 78.3762
      }
    ]
  }
}
```

---

## 2. Create Expo Event (UPDATED)
Creates a new expo event. **Note:** If `lat` and `lng` are provided, the backend will automatically find users near the location (50km radius) and send them an in-app notification of type `"EXPO"`.

**Endpoint:** `POST /api/expo`
**Headers:** `Authorization: Bearer <JWT_TOKEN>`

**Request Body:**
```json
{
  "title": "Luxury Real Estate Expo 2026",
  "description": "Discover premium properties...",
  "location": "HITEC City, Hyderabad",
  "event_date": "2026-05-10",
  "start_time": "10:00",
  "end_time": "18:00",
  "organizer_id": "550e8400-e29b-41d4-a716-446655440000",
  "banner_image": "https://bucket.s3.amazonaws.com/image.png",
  "max_participants": 500,
  "lat": 17.4474,   // NEW: Optional (Highly recommended for notifications)
  "lng": 78.3762    // NEW: Optional
}
```

---

## 3. Edit Expo Event (NEW)
Performs a partial update on an existing expo event. Only the creator/organizer of the event is authorized to perform this action. You only need to send the fields you want to update.

**Endpoint:** `PUT /api/expo/{expo_id}`
**Headers:** `Authorization: Bearer <JWT_TOKEN>`

**Request Body:** *(All fields are optional)*
```json
{
  "title": "Updated Expo Title",
  "description": "Updated description...",
  "location": "New Location Address",
  "event_date": "2026-06-15",
  "start_time": "11:00",
  "end_time": "19:00",
  "banner_image": "https://bucket.s3.amazonaws.com/new_banner.png",
  "max_participants": 600, // Cannot be set lower than current registered users
  "lat": 17.4500,          // NEW
  "lng": 78.3800           // NEW
}
```

**Response (200 OK):**
```json
{
  "success": true,
  "message": "Expo event updated successfully",
  "data": {
    "expo_id": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

*(Note: The global `GET /api/expo` list and `GET /api/expo/{expo_id}` detail endpoints will now also return `lat` and `lng` alongside the other properties).*
