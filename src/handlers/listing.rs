use axum::{
    http::StatusCode,
    response::Json,
    extract::{State, Path},
};

use crate::app_state::AppState;

use serde_json::json;
use crate::dtos::request::{CreateListingRequest, UpdateListingRequest};

/// Get all listings
#[utoipa::path(
    get,
    path = "/api/listings",
    params(
        ("page" = Option<i32>, Query, description = "Page number"),
        ("limit" = Option<i32>, Query, description = "Items per page"),
        ("city" = Option<String>, Query, description = "Filter by city"),
        ("price_min" = Option<i32>, Query, description = "Minimum price"),
        ("price_max" = Option<i32>, Query, description = "Maximum price"),
        ("accommodation" = Option<String>, Query, description = "Filter by accommodation type")
    ),
    responses(
        (status = 200, description = "Listings retrieved successfully", body = ApiResponse<ListingsResponse>),
        (status = 400, description = "Bad request")
    ),
    tag = "Property Listings"
)]
pub async fn get_listings(
    State(_app_state): State<AppState>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement get all listings logic
    // 1. Get listings from database with pagination
    // 2. Apply filters (city, price range, etc.)
    // 3. Return listings

    let response = json!({
        "success": true,
        "message": "Listings retrieved successfully",
        "data": {
            "listings": [
                {
                    "id": "456e7890-e89b-12d3-a456-426614174001",
                    "title": "Modern 2BHK Apartment in City Center",
                    "description": "Beautiful 2BHK apartment with modern amenities, located in the heart of the city. Perfect for working professionals.",
                    "city": "Mumbai",
                    "area": "Bandra West",
                    "pincode": "400050",
                    "accommodation": "Private",
                    "apartment_type": "2BHK",
                    "roommates": 0,
                    "gender_preference": "Any",
                    "carpet_area": 1200,
                    "bathrooms": 2,
                    "price": 25000,
                    "label": "Premium",
                    "likes": 45,
                    "host": "Premium Properties",
                    "is_featured": true,
                    "user_id": "123e4567-e89b-12d3-a456-426614174000",
                    "images": [
                        "https://example.com/listing1_img1.jpg",
                        "https://example.com/listing1_img2.jpg"
                    ],
                    "status": "active",
                    "views": 156,
                    "shares": 12,
                    "broker_commission": null,
                    "is_broker_verified": false,
                    "broker_contact_allowed": true,
                    "priority_listing": false,
                    "listing_type": "direct",
                    "created_at": "2024-01-10T09:00:00Z"
                },
                {
                    "id": "456e7890-e89b-12d3-a456-426614174002",
                    "title": "Cozy 1BHK Studio Near Metro",
                    "description": "Compact and well-maintained 1BHK studio apartment, just 5 minutes walk from metro station.",
                    "city": "Delhi",
                    "area": "Dwarka",
                    "pincode": "110075",
                    "accommodation": "Private",
                    "apartment_type": "1BHK",
                    "roommates": 0,
                    "gender_preference": "Any",
                    "carpet_area": 800,
                    "bathrooms": 1,
                    "price": 18000,
                    "label": "Budget Friendly",
                    "likes": 23,
                    "host": "Metro Properties",
                    "is_featured": false,
                    "user_id": "123e4567-e89b-12d3-a456-426614174000",
                    "images": [
                        "https://example.com/listing2_img1.jpg"
                    ],
                    "status": "active",
                    "views": 89,
                    "shares": 5,
                    "broker_commission": 2.5,
                    "is_broker_verified": true,
                    "broker_contact_allowed": true,
                    "priority_listing": true,
                    "listing_type": "brokered",
                    "created_at": "2024-01-12T14:30:00Z"
                }
            ],
            "pagination": {
                "page": 1,
                "limit": 10,
                "total": 2,
                "total_pages": 1
            }
        }
    });

    (StatusCode::OK, Json(response))
}

/// Create new listing
#[utoipa::path(
    post,
    path = "/api/listings",
    request_body = CreateListingRequest,
    responses(
        (status = 201, description = "Listing created successfully", body = ApiResponse<ListingResponse>),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized")
    ),
    tag = "Property Listings"
)]
pub async fn create_listing(
    State(_app_state): State<AppState>,
    Json(_payload): Json<CreateListingRequest>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement create listing logic
    // 1. Extract user from JWT token
    // 2. Validate listing data
    // 3. Create listing in database
    // 4. Return created listing

    let response = json!({
        "success": true,
        "message": "Listing created successfully",
        "data": {
            "listing": {
                "id": "456e7890-e89b-12d3-a456-426614174003",
                "title": "New 3BHK Apartment with Garden",
                "description": "Spacious 3BHK apartment with beautiful garden view, modern amenities, and 24/7 security.",
                "city": "Bangalore",
                "area": "Whitefield",
                "pincode": "560066",
                "accommodation": "Private",
                "apartment_type": "3BHK",
                "roommates": 0,
                "gender_preference": "Any",
                "carpet_area": 1800,
                "bathrooms": 3,
                "price": 35000,
                "label": "Luxury",
                "likes": 0,
                "host": "Luxury Homes",
                "is_featured": false,
                "user_id": "123e4567-e89b-12d3-a456-426614174000",
                "images": [
                    "https://example.com/listing3_img1.jpg",
                    "https://example.com/listing3_img2.jpg",
                    "https://example.com/listing3_img3.jpg"
                ],
                "status": "active",
                "views": 0,
                "shares": 0,
                "broker_commission": null,
                "is_broker_verified": false,
                "broker_contact_allowed": true,
                "priority_listing": false,
                "listing_type": "direct",
                "created_at": "2024-01-15T15:00:00Z"
            }
        }
    });

    (StatusCode::CREATED, Json(response))
}

/// Get listing by ID
#[utoipa::path(
    get,
    path = "/api/listings/{id}",
    params(
        ("id" = String, Path, description = "Listing ID")
    ),
    responses(
        (status = 200, description = "Listing retrieved successfully", body = ApiResponse<ListingResponse>),
        (status = 404, description = "Listing not found")
    ),
    tag = "Property Listings"
)]
pub async fn get_listing_by_id(
    State(_app_state): State<AppState>,
    Path(_id): Path<String>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement get listing by ID logic
    // 1. Find listing by ID
    // 2. Increment view count
    // 3. Return listing details

    let response = json!({
        "success": true,
        "message": "Listing retrieved successfully",
        "data": {
            "listing": {
                "id": "456e7890-e89b-12d3-a456-426614174001",
                "title": "Modern 2BHK Apartment in City Center",
                "description": "Beautiful 2BHK apartment with modern amenities, located in the heart of the city. Perfect for working professionals. Features include: fully furnished, modular kitchen, 2 balconies, parking space, 24/7 security, gym, swimming pool, and children's play area.",
                "city": "Mumbai",
                "area": "Bandra West",
                "pincode": "400050",
                "accommodation": "Private",
                "apartment_type": "2BHK",
                "roommates": 0,
                "gender_preference": "Any",
                "carpet_area": 1200,
                "bathrooms": 2,
                "price": 25000,
                "label": "Premium",
                "likes": 45,
                "host": "Premium Properties",
                "is_featured": true,
                "user_id": "123e4567-e89b-12d3-a456-426614174000",
                "images": [
                    "https://example.com/listing1_img1.jpg",
                    "https://example.com/listing1_img2.jpg",
                    "https://example.com/listing1_img3.jpg",
                    "https://example.com/listing1_img4.jpg"
                ],
                "status": "active",
                "views": 157,
                "shares": 12,
                "broker_commission": null,
                "is_broker_verified": false,
                "broker_contact_allowed": true,
                "priority_listing": false,
                "listing_type": "direct",
                "created_at": "2024-01-10T09:00:00Z",
                "updated_at": "2024-01-15T16:30:00Z",
                "host_details": {
                    "id": "123e4567-e89b-12d3-a456-426614174000",
                    "name": "Premium Properties",
                    "phone": "9876543210",
                    "email": "contact@premiumproperties.com",
                    "rating": 4.5,
                    "verified": true
                }
            }
        }
    });

    (StatusCode::OK, Json(response))
}

/// Update listing
#[utoipa::path(
    put,
    path = "/api/listings/{id}",
    params(
        ("id" = String, Path, description = "Listing ID")
    ),
    request_body = UpdateListingRequest,
    responses(
        (status = 200, description = "Listing updated successfully", body = ApiResponse<ListingResponse>),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - not the owner"),
        (status = 404, description = "Listing not found")
    ),
    tag = "Property Listings"
)]
pub async fn update_listing(
    State(_app_state): State<AppState>,
    Path(_id): Path<String>,
    Json(_payload): Json<UpdateListingRequest>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement update listing logic
    // 1. Extract user from JWT token
    // 2. Verify user owns the listing
    // 3. Update listing in database
    // 4. Return updated listing

    let response = json!({
        "success": true,
        "message": "Listing updated successfully",
        "data": {
            "listing": {
                "id": "456e7890-e89b-12d3-a456-426614174001",
                "title": "Updated Modern 2BHK Apartment in City Center",
                "description": "Beautiful 2BHK apartment with modern amenities, located in the heart of the city. Perfect for working professionals. Recently renovated with new furniture.",
                "city": "Mumbai",
                "area": "Bandra West",
                "pincode": "400050",
                "accommodation": "Private",
                "apartment_type": "2BHK",
                "roommates": 0,
                "gender_preference": "Any",
                "carpet_area": 1200,
                "bathrooms": 2,
                "price": 28000,
                "label": "Premium",
                "likes": 45,
                "host": "Premium Properties",
                "is_featured": true,
                "user_id": "123e4567-e89b-12d3-a456-426614174000",
                "images": [
                    "https://example.com/listing1_img1.jpg",
                    "https://example.com/listing1_img2.jpg",
                    "https://example.com/listing1_img3.jpg"
                ],
                "status": "active",
                "views": 157,
                "shares": 12,
                "broker_commission": null,
                "is_broker_verified": false,
                "broker_contact_allowed": true,
                "priority_listing": false,
                "listing_type": "direct",
                "created_at": "2024-01-10T09:00:00Z",
                "updated_at": "2024-01-15T17:00:00Z"
            }
        }
    });

    (StatusCode::OK, Json(response))
}

/// Delete listing
#[utoipa::path(
    delete,
    path = "/api/listings/{id}",
    params(
        ("id" = String, Path, description = "Listing ID")
    ),
    responses(
        (status = 200, description = "Listing deleted successfully", body = ApiResponse<serde_json::Value>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - not the owner"),
        (status = 404, description = "Listing not found")
    ),
    tag = "Property Listings"
)]
pub async fn delete_listing(
    State(_app_state): State<AppState>,
    Path(_id): Path<String>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement delete listing logic
    // 1. Extract user from JWT token
    // 2. Verify user owns the listing
    // 3. Soft delete or hard delete listing
    // 4. Return success response

    let response = json!({
        "success": true,
        "message": "Listing deleted successfully",
        "data": {
            "deleted_listing_id": "456e7890-e89b-12d3-a456-426614174001",
            "deleted_at": "2024-01-15T18:00:00Z"
        }
    });

    (StatusCode::OK, Json(response))
}

/// Like listing
#[utoipa::path(
    post,
    path = "/api/listings/{id}/like",
    params(
        ("id" = String, Path, description = "Listing ID")
    ),
    responses(
        (status = 200, description = "Listing liked successfully", body = ApiResponse<serde_json::Value>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Listing not found")
    ),
    tag = "Property Listings"
)]
pub async fn like_listing(
    State(_app_state): State<AppState>,
    Path(_id): Path<String>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement like listing logic
    // 1. Extract user from JWT token
    // 2. Add user to liked_by array
    // 3. Increment likes count
    // 4. Return success response

    let response = json!({
        "success": true,
        "message": "Listing liked successfully",
        "data": {
            "listing_id": "456e7890-e89b-12d3-a456-426614174001",
            "user_id": "123e4567-e89b-12d3-a456-426614174000",
            "liked": true,
            "total_likes": 46,
            "liked_at": "2024-01-15T19:00:00Z"
        }
    });

    (StatusCode::OK, Json(response))
}

/// Save listing
#[utoipa::path(
    post,
    path = "/api/listings/{id}/save",
    params(
        ("id" = String, Path, description = "Listing ID")
    ),
    responses(
        (status = 200, description = "Listing saved successfully", body = ApiResponse<serde_json::Value>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Listing not found")
    ),
    tag = "Property Listings"
)]
pub async fn save_listing(
    State(_app_state): State<AppState>,
    Path(_id): Path<String>,
) -> impl axum::response::IntoResponse {
    // TODO: Implement save listing logic
    // 1. Extract user from JWT token
    // 2. Add listing to user's saved_listings
    // 3. Add user to listing's saved_by array
    // 4. Return success response

    let response = json!({
        "success": true,
        "message": "Listing saved successfully",
        "data": {
            "listing_id": "456e7890-e89b-12d3-a456-426614174001",
            "user_id": "123e4567-e89b-12d3-a456-426614174000",
            "saved": true,
            "total_saves": 8,
            "saved_at": "2024-01-15T20:00:00Z"
        }
    });

    (StatusCode::OK, Json(response))
}