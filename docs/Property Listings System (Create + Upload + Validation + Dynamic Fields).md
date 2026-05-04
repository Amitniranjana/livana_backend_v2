Subject: Backend Update: New Unified Property Listings API (v2) is Ready

The new Unified Property Listings API is now fully implemented and verified. This update introduces a consolidated approach to handling listings for Users, Brokers, and Associates, alongside dynamic validation rules based on property types.

Here are the details you need for integration:

🚀 New Endpoints
1. Create Listing

Method: POST /api/listings
Auth: Requires JWT
Payload structure: Standardized format (see CreateListingPayload DTO).
Key Behavior: The backend now enforces dynamic validation:
Residential: Requires bedrooms, bathrooms, no_of_toilets, no_of_balconies.
Commercial: Rejects residential fields.
Land: Rejects building-specific fields.
Space Sharing / PG: Requires gender_preference and roommates.
Sell: The deposit must be exactly 0.
Note: Standard Users cannot create "Sell" listings; only Brokers and Associates can.
2. List Properties (with Filtering & Pagination)

Method: GET /api/listings
Auth: Public
Query Parameters:
city, property_type, listing_type, user_type, bedrooms
min_price, max_price
limit (default 20, max 100), offset (default 0)
sort_by (price_asc, price_desc, latest)
Response: Includes a pagination object (total, limit, offset, has_more) alongside the listings array.
3. Get Listing Details

Method: GET /api/listings/{id}
Auth: Public
Performance: Backed by a Redis cache (60s TTL) for fast loading times.
Response: Returns full property details, including an ordered array of images.
4. Upload Listing Images (S3 Multipart)

Method: POST /api/listings/v2/upload/images
Auth: Requires JWT
Format: multipart/form-data
Limits: Max 10 images per request, 5MB limit per file. Only JPEG, PNG, and WEBP are supported.
Usage: You can pass an optional listing_id field in the form data if the listing already exists. Otherwise, upload the images first, get the URLs from the response, and include them in the image_urls array when calling POST /api/listings.
💡 Notable Business Logic
Auto-derive Parking: If "Parking" or "Parking lot" is selected in the amenities array, the backend automatically sets parking = true.
The API is backward compatible; the legacy /api/properties endpoints are still functional, but we recommend migrating to the new /api/listings endpoints for all new development.

Let me know if you need any API