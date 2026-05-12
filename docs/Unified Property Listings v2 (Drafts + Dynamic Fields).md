# Unified Property Listings API v2 (Drafts & Dynamic Fields)

This documentation covers the newly updated Unified Property Listings API, including the new Drafts feature, dynamic `{n}BHK` support, and conditional rules for different listing types.

## 1. Create Listing
**Endpoint**: `POST /api/listings`
**Auth**: Requires JWT

This endpoint handles the final submission of a property listing. It performs strict validation on all required fields according to the business rules.

### Request Payload Structure
```json
{
  "title": "Modern 2BHK Apartment",
  "description": "Beautiful apartment in the city center.",
  "property_type": "2BHK", 
  "price": 25000,
  "deposit": 50000,
  "location": "Bandra West, Mumbai",
  "area_sqft": 1200,
  "bedrooms": 2,
  "bathrooms": 2,
  "no_of_toilets": 2,
  "no_of_balconies": 1,
  "images": ["https://s3.../image1.jpg", "https://s3.../image2.jpg"],
  "host": "User",
  "user_type": "user",
  "listing_type": "Rent",
  "amenities": ["Gym", "Parking lot", "Water"],
  "furnishing": "Semi-Furnished",
  "parking": true,
  "broker_contact_allowed": true,

  // Conditionally included (omit when null/empty)
  "area": "Bandra",
  "city": "Mumbai",
  "pincode": "400050",
  "latitude": 19.0596,
  "longitude": 72.8295,
  "floor": 5,
  "total_floors": 12,
  "age_years": 3,
  "facing": "East",

  // Space Sharing / PG only
  "gender_preference": "Any",
  "roommates": 2,

  // Lease only
  "lease_years": 3,

  // Commercial only
  "bathroom_type": "Attached"
}
```

### Key Business Rules & Validation
1. **Dynamic Property Types**: `property_type` accepts `"Studio"`, `"Commercial"`, `"Land"`, or any string ending with `"BHK"` (e.g., `"1BHK"`, `"2BHK"`, `"4BHK"`).
2. **Deposit Rules**: For `Sell` and `Land` listings, `deposit` **must** be exactly `0`.
3. **User Type Restrictions**: Regular users (`user_type` = `"user"`) **cannot** create `"Sell"` listings. Only Brokers and Associates can.
4. **Residential Validation** (`Studio` or `*BHK`): Requires `bedrooms`, `bathrooms`, `no_of_toilets`, and `no_of_balconies`.
5. **Commercial Validation**: Rejects residential fields (`bedrooms`, `bathrooms`, `furnishing`, etc.). If provided, `bathroom_type` must be `"Attached"` or `"Common"`.
6. **Space Sharing & PG**: Requires `gender_preference` and `roommates`.
7. **Lease Validation**: If `listing_type` is `"Lease"`, `lease_years` must be provided.
8. **Auto Parking**: If `"Parking"` or `"Parking lot"` is passed in `amenities`, the backend automatically sets `parking` to `true`.

---

## 2. Save Listing Draft
**Endpoint**: `POST /api/listings/drafts`
**Auth**: Requires JWT

Use this endpoint when a user starts creating a listing but wants to save their progress before they have all the required fields. 

### Request Payload Structure
The draft API accepts a completely free-form JSON object inside the `data` key. It does **not** run strict validations like `POST /api/listings`.

```json
{
  "data": {
    "title": "Modern 2BHK Apartment",
    "property_type": "2BHK",
    "price": 25000,
    "step": 2, // You can store frontend state/step tracking here
    // ... any other partial data
  }
}
```

### Response
```json
{
  "success": true,
  "data": {
    "draft_id": "c1f7a2d4-1a3b-4c4d-8e9f-0a1b2c3d4e5f"
  }
}
```

---

## 3. Get Listing Drafts
**Endpoint**: `GET /api/listings/drafts`
**Auth**: Requires JWT

Fetches all saved drafts for the currently authenticated user, ordered by `updated_at` descending (newest first).

### Response
```json
{
  "success": true,
  "data": [
    {
      "id": "c1f7a2d4-1a3b-4c4d-8e9f-0a1b2c3d4e5f",
      "user_id": "a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d",
      "data": {
        "title": "Modern 2BHK Apartment",
        "property_type": "2BHK",
        "price": 25000,
        "step": 2
      },
      "created_at": "2026-05-12T16:00:00Z",
      "updated_at": "2026-05-12T16:05:00Z"
    }
  ]
}
```

---

## 4. Upload Listing Images (S3)
**Endpoint**: `POST /api/listings/v2/upload/images`
**Auth**: Requires JWT
**Format**: `multipart/form-data`

Upload property images to S3 before creating the listing.
- **Limits**: Max 10 images, 5MB per file. Only JPEG, PNG, and WEBP supported.
- **Fields**: Pass files under `files` or `images` keys. Optionally pass `listing_id` if uploading to an existing listing.
- **Usage**: Extract the returned URLs and pass them in the `images` array of the `POST /api/listings` endpoint.
