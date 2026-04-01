use serde::Serialize;

use utoipa::ToSchema;

// Request/Response DTOs

#[allow(dead_code)]
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct SignupUserData {
    pub id: uuid::Uuid,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone_no: String,
    pub user_role: String,
    pub verified: bool,
    pub is_phone_verified: bool,
    pub status: String,
    pub associate_type: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct SignupResponseData {
    pub token: String,
    pub user: SignupUserData,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")]
    pub token: String,
    pub user_id: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[allow(dead_code)]
pub struct UserResponse {
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub id: String,
    #[schema(example = "John")]
    pub first_name: String,
    #[schema(example = "Doe")]
    pub last_name: String,
    #[schema(example = "john.doe@example.com")]
    pub email: String,
    #[schema(example = "1234567890")]
    pub phone_no: String,
    #[schema(example = "male")]
    pub gender: String,
    #[schema(example = "user")]
    pub user_role: String,
    pub verified: bool,
    #[schema(example = "https://example.com/profile.jpg")]
    pub profile_image_url: Option<String>,
    #[schema(example = "Software developer with 5 years of experience")]
    pub bio: Option<String>,
    #[schema(example = "Premium Properties")]
    pub business_name: Option<String>,
    #[schema(example = "BRK123456")]
    pub license_number: Option<String>,
    #[schema(example = 5)]
    pub experience_years: Option<i32>,
    #[schema(example = 2.5)]
    pub commission_rate: Option<f64>,
    #[schema(example = 4.5)]
    pub broker_rating: Option<f64>,
    #[schema(example = 25)]
    pub total_reviews: Option<i32>,
    pub is_verified_broker: bool,
    #[schema(example = "active")]
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[allow(dead_code)]
pub struct ListingResponse {
    #[schema(example = "456e7890-e89b-12d3-a456-426614174001")]
    pub id: String,
    #[schema(example = "Modern 2BHK Apartment in City Center")]
    pub title: String,
    #[schema(example = "Beautiful 2BHK apartment with modern amenities.")]
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
    pub images: Option<Vec<String>>,
    #[schema(example = "active")]
    pub status: String,
    #[schema(example = 156)]
    pub views: i32,
    #[schema(example = 12)]
    pub shares: i32,
    #[schema(example = 2.5)]
    pub broker_commission: Option<f64>,
    pub is_broker_verified: bool,
    pub broker_contact_allowed: bool,
    pub priority_listing: bool,
    #[schema(example = "direct")]
    pub listing_type: Option<String>,
    pub created_at: String,
}

// Common response structures
#[derive(Debug, Serialize, ToSchema)]
#[allow(dead_code)]
pub struct ApiResponse<T> {
    #[schema(example = true)]
    pub success: bool,
    #[schema(example = "Operation completed successfully")]
    pub message: String,
    #[schema(example = "you can write your message here")]
    pub data: T,
}
#[derive(Debug, Serialize, ToSchema)]
#[allow(dead_code)]
pub struct ApiAuthData {
    pub user: ApiResponse<UserResponse>,
    pub token: String,
    pub refresh_token: String,
}
#[derive(Debug, Serialize, ToSchema)]
#[allow(dead_code)]
pub struct PaginationInfo {
    #[schema(example = 1)]
    pub page: i32,
    #[schema(example = 10)]
    pub limit: i32,
    #[schema(example = 100)]
    pub total: i32,
    #[schema(example = 10)]
    pub total_pages: i32,
}

#[derive(Debug, Serialize, ToSchema)]
#[allow(dead_code)]
pub struct ListingsResponse {
    pub listings: Vec<ListingResponse>,
    pub pagination: PaginationInfo,
}
