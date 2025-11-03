use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct SignupRequest {
    #[schema(example = "John")]
    pub first_name: String,
    #[schema(example = "Doe")]
    pub last_name: String,
    #[schema(example = "john.doe@example.com")]
    pub email: String,
    #[schema(example = "password123")]
    pub password: String,
    #[schema(example = "1234567890")]
    pub phone_no: String,
    #[schema(example = "male")]
    pub gender: String,
    #[schema(example = "user")]
    pub user_role: String,
    #[schema(example = "Premium Properties")]
    pub business_name: Option<String>,
    #[schema(example = "BRK123456")]
    pub license_number: Option<String>,
    #[schema(example = 5)]
    pub experience_years: Option<i32>,
    #[schema(example = 2.5)]
    pub commission_rate: Option<f64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SigninRequest {
    #[schema(example = "john.doe@example.com")]
    pub email: String,
    #[schema(example = "password123")]
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ForgotPasswordRequest {
    #[schema(example = "john.doe@example.com")]
    pub email: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResetPasswordRequest {
    #[schema(example = "john.doe@example.com")]
    pub email: String,
    #[schema(example = "123456")]
    pub code: String,
    #[schema(example = "newpassword123")]
    pub new_password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateProfileRequest {
    #[schema(example = "John")]
    pub first_name: Option<String>,
    #[schema(example = "Smith")]
    pub last_name: Option<String>,
    #[schema(example = "9876543210")]
    pub phone_no: Option<String>,
    #[schema(example = "male")]
    pub gender: Option<String>,
    #[schema(example = "Updated bio: Full-stack developer passionate about creating user-friendly applications")]
    pub bio: Option<String>,
    #[schema(example = "Premium Properties")]
    pub business_name: Option<String>,
    #[schema(example = 5)]
    pub experience_years: Option<i32>,
    #[schema(example = 2.5)]
    pub commission_rate: Option<f64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateListingRequest {
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
    #[schema(example = "Premium Properties")]
    pub host: String,
    pub images: Option<Vec<String>>,
    pub location: Option<serde_json::Value>,
    #[schema(example = 2.5)]
    pub broker_commission: Option<f64>,
    #[schema(example = "direct")]
    pub listing_type: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateListingRequest {
    #[schema(example = "Updated Modern 2BHK Apartment in City Center")]
    pub title: Option<String>,
    #[schema(example = "Beautiful 2BHK apartment with modern amenities, recently renovated.")]
    pub description: Option<String>,
    #[schema(example = "Mumbai")]
    pub city: Option<String>,
    #[schema(example = "Bandra West")]
    pub area: Option<String>,
    #[schema(example = "400050")]
    pub pincode: Option<String>,
    #[schema(example = "Private")]
    pub accommodation: Option<String>,
    #[schema(example = "2BHK")]
    pub apartment_type: Option<String>,
    #[schema(example = 0)]
    pub roommates: Option<i32>,
    #[schema(example = "Any")]
    pub gender_preference: Option<String>,
    #[schema(example = 1200)]
    pub carpet_area: Option<i32>,
    #[schema(example = 2)]
    pub bathrooms: Option<i32>,
    #[schema(example = 28000)]
    pub price: Option<i32>,
    #[schema(example = "Premium")]
    pub label: Option<String>,
    #[schema(example = "Premium Properties")]
    pub host: Option<String>,
    pub images: Option<Vec<String>>,
    pub location: Option<serde_json::Value>,
    #[schema(example = "active")]
    pub status: Option<String>,
}