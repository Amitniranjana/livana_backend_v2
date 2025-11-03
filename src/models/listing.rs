use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Listing {
    #[schema(example = "456e7890-e89b-12d3-a456-426614174001")]
    pub id: String,
    #[schema(example = "Modern 2BHK Apartment in City Center")]
    pub title: String,
    #[schema(example = "Beautiful 2BHK apartment with modern amenities, located in the heart of the city.")]
    pub description: String,
    #[schema(example = "Mumbai")]
    pub city: String,
    #[schema(example = "Bandra West")]
    pub area: String,
    #[schema(example = "400050")]
    pub pincode: String,
    #[schema(example = "Private")]
    pub accommodation: String,
    #[schema(example = "2BHK")]
    pub apartment_type: String,
    #[schema(example = 0)]
    pub roommates: i32,
    #[schema(example = "Any")]
    pub gender_preference: String,
    #[schema(example = 1200)]
    pub carpet_area: i32,
    #[schema(example = 2)]
    pub bathrooms: i32,
    #[schema(example = 25000)]
    pub price: i32,
    #[schema(example = "Premium")]
    pub label: Option<String>,
    #[schema(example = 45)]
    pub likes: i32,
    #[schema(example = "Premium Properties")]
    pub host: String,
    pub is_featured: bool,
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub user_id: String,
    pub images: Option<serde_json::Value>,
    pub saved_by: Option<serde_json::Value>,
    pub liked_by: Option<serde_json::Value>,
    pub location: Option<serde_json::Value>,
    #[schema(example = "active")]
    pub status: String,
    #[schema(example = 156)]
    pub views: i32,
    #[schema(example = 12)]
    pub shares: i32,
    // Broker-specific fields
    #[schema(example = 2.5)]
    pub broker_commission: Option<f64>,
    pub is_broker_verified: bool,
    pub broker_contact_allowed: bool,
    pub priority_listing: bool,
    #[schema(example = "direct")]
    pub listing_type: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
