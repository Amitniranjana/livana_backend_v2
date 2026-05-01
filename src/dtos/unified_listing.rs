use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Request payload for POST /api/listings
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateListingPayload {
    pub title: String,
    pub description: String,

    /// Residential | Commercial | Land
    pub property_type: String,
    /// Rent | Sell | PG | Space Sharing
    pub listing_type: String,
    /// User | Broker | Associate
    pub user_type: String,

    pub price: i32,
    pub deposit: i32,

    pub location: String,
    pub area: Option<String>,
    pub city: Option<String>,
    pub pincode: Option<String>,

    pub latitude: Option<f64>,
    pub longitude: Option<f64>,

    pub area_sqft: i32,

    pub bedrooms: Option<i32>,
    pub bathrooms: Option<i32>,
    pub no_of_toilets: Option<i32>,
    pub no_of_balconies: Option<i32>,

    pub furnishing: Option<String>,
    pub facing: Option<String>,

    pub floor: Option<i32>,
    pub total_floors: Option<i32>,

    pub commercial_type: Option<String>,
    pub land_type: Option<String>,

    pub gender_preference: Option<String>,
    pub roommates: Option<i32>,

    pub amenities: Option<Vec<String>>,
    pub parking: Option<bool>,
    pub broker_contact_allowed: Option<bool>,

    pub age_years: Option<i32>,

    /// Image URLs returned from the upload endpoint
    pub image_urls: Option<Vec<String>>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Query params for GET /api/listings
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListingFilters {
    pub city: Option<String>,
    pub property_type: Option<String>,
    pub listing_type: Option<String>,
    pub user_type: Option<String>,
    pub min_price: Option<i32>,
    pub max_price: Option<i32>,
    pub bedrooms: Option<i32>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub sort_by: Option<String>, // price_asc | price_desc | latest
}

// ─────────────────────────────────────────────────────────────────────────────
// Response types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ListingImageRow {
    pub id: String,
    pub image_url: String,
    pub display_order: Option<i32>,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListingDetail {
    pub id: String,
    pub title: String,
    pub description: String,

    pub property_type: String,
    pub listing_type: String,
    pub user_type: String,

    pub price: i32,
    pub deposit: i32,

    pub location: String,
    pub area: Option<String>,
    pub city: Option<String>,
    pub pincode: Option<String>,

    pub latitude: Option<f64>,
    pub longitude: Option<f64>,

    pub area_sqft: i32,

    pub bedrooms: Option<i32>,
    pub bathrooms: Option<i32>,
    pub no_of_toilets: Option<i32>,
    pub no_of_balconies: Option<i32>,

    pub furnishing: Option<String>,
    pub facing: Option<String>,

    pub floor: Option<i32>,
    pub total_floors: Option<i32>,

    pub commercial_type: Option<String>,
    pub land_type: Option<String>,

    pub gender_preference: Option<String>,
    pub roommates: Option<i32>,

    pub amenities: Option<Vec<String>>,
    pub parking: Option<bool>,
    pub broker_contact_allowed: Option<bool>,

    pub age_years: Option<i32>,

    pub created_by: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,

    pub images: Vec<ListingImageRow>,
}

#[derive(Debug, Serialize)]
pub struct ListingSummary {
    pub id: String,
    pub title: String,
    pub property_type: String,
    pub listing_type: String,
    pub user_type: String,
    pub price: i32,
    pub deposit: i32,
    pub location: String,
    pub city: Option<String>,
    pub area_sqft: i32,
    pub bedrooms: Option<i32>,
    pub bathrooms: Option<i32>,
    pub parking: Option<bool>,
    pub created_at: Option<String>,
    pub image_count: i64,
}
