use serde::Deserialize;
use utoipa::ToSchema;
fn default_user_role() -> String {
    "user".to_string()
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
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
    #[serde(default = "default_user_role")]
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
#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SigninRequest {
    #[schema(example = "john.doe@example.com")]
    pub email: Option<String>,
    #[schema(example = "+919876543210")]
    pub phone_no: Option<String>,
    #[schema(example = "password123")]
    pub password: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SendOtpRequest {
    #[schema(example = "+919876543210")]
    pub phone_no: Option<String>,
    #[schema(example = "john.doe@example.com")]
    pub email: Option<String>,
    /// Purpose of the OTP: "signup", "forgot_password", "change_password"
    #[schema(example = "signup")]
    pub purpose: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerifyOtpRequest {
    #[schema(example = "+919876543210")]
    pub phone_no: Option<String>,
    #[schema(example = "john.doe@example.com")]
    pub email: Option<String>,
    #[schema(example = "123456")]
    pub otp: String,
    /// Purpose of the OTP: "signup", "forgot_password", "change_password"
    #[schema(example = "signup")]
    pub purpose: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAssociateTypeRequest {
    #[schema(example = "broker")]
    pub associate_type: String,
}
#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ForgotPasswordRequest {
    #[schema(example = "john.doe@example.com")]
    pub email: Option<String>,
    #[schema(example = "+919876543210")]
    pub phone_no: Option<String>,
}
#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
pub struct ResetPasswordRequest {
    #[schema(example = "john.doe@example.com")]
    pub email: String,
    #[schema(example = "123456")]
    pub code: String,
    #[schema(example = "newpassword123")]
    pub new_password: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    #[schema(example = "oldpassword123")]
    pub current_password: String,
    #[schema(example = "newpassword123")]
    pub new_password: String,
}
#[allow(dead_code)]
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
    #[schema(
        example = "Updated bio: Full-stack developer passionate about creating user-friendly applications"
    )]
    pub bio: Option<String>,
    #[schema(example = "Premium Properties")]
    pub business_name: Option<String>,
    #[schema(example = 5)]
    pub experience_years: Option<i32>,
    #[schema(example = 2.5)]
    pub commission_rate: Option<f64>,
}
/// --- Property Create/Update DTOs ---
#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreatePropertyRequest {
    #[schema(example = "Modern 2BHK Apartment in City Center")]
    pub title: String,
    #[schema(example = "Beautiful 2BHK apartment with modern amenities.")]
    pub description: Option<String>,
    #[schema(example = "rent")]
    pub property_type: String,
    #[schema(example = "Rent")]
    pub listing_type: Option<String>,
    #[schema(example = 25000)]
    pub price: i64,
    #[schema(example = 50000)]
    pub deposit: Option<i64>,
    #[schema(example = "Bandra West, Mumbai")]
    pub location: Option<String>,
    #[schema(example = 1200)]
    pub area_sqft: Option<i32>,
    #[schema(example = 2)]
    pub bedrooms: Option<i32>,
    #[schema(example = 2)]
    pub bathrooms: Option<i32>,
    #[schema(example = 2)]
    pub no_of_toilets: Option<i32>,
    #[schema(example = 1)]
    pub no_of_balconies: Option<i32>,
    /// "unfurnished" | "semi-furnished" | "fully-furnished"
    #[schema(example = "semi-furnished")]
    pub furnishing: Option<String>,
    #[schema(example = 5)]
    pub floor: Option<i32>,
    #[schema(example = 12)]
    pub total_floors: Option<i32>,
    #[schema(example = 3)]
    pub age_years: Option<i32>,
    /// "north" | "south" | "east" | "west" etc.
    #[schema(example = "east")]
    pub facing: Option<String>,
    #[schema(example = true)]
    pub parking: Option<bool>,
    #[schema(example = 1)]
    pub parking_count: Option<i32>,
    pub images: Option<Vec<String>>,
    #[schema(example = "https://example.com/video.mp4")]
    pub video_url: Option<String>,
    pub amenities: Option<Vec<String>>,
    pub nearby_places: Option<serde_json::Value>,
    #[schema(example = 19.0760)]
    pub latitude: Option<f64>,
    #[schema(example = 72.8777)]
    pub longitude: Option<f64>,
    #[schema(example = "Premium Properties")]
    pub host: Option<String>,
    #[schema(example = "Premium")]
    pub label: Option<String>,
    #[schema(example = "user")]
    pub user_type: Option<String>,
    pub broker_contact_allowed: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)]
pub struct UpdatePropertyRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    /// "rent" | "sale" | "pg"
    pub property_type: Option<String>,
    pub price: Option<i64>,
    pub deposit: Option<i64>,
    pub location: Option<String>,
    pub area_sqft: Option<i32>,
    pub bedrooms: Option<i32>,
    pub bathrooms: Option<i32>,
    pub no_of_toilets: Option<i32>,
    pub no_of_balconies: Option<i32>,
    pub furnishing: Option<String>,
    pub floor: Option<i32>,
    pub total_floors: Option<i32>,
    pub age_years: Option<i32>,
    pub facing: Option<String>,
    pub parking: Option<bool>,
    pub parking_count: Option<i32>,
    pub images: Option<Vec<String>>,
    pub video_url: Option<String>,
    pub amenities: Option<Vec<String>>,
    pub nearby_places: Option<serde_json::Value>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub host: Option<String>,
    pub label: Option<String>,
    pub status: Option<String>,
    pub user_type: Option<String>,
    pub broker_contact_allowed: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)]
pub struct ReportPropertyRequest {
    #[schema(example = "spam")]
    pub reason: String,
    #[schema(example = "This listing appears to be fraudulent.")]
    pub description: Option<String>,
}

/// Keep old types as aliases so other handlers that still use them don't break
#[allow(dead_code)]
pub type CreateListingRequest = CreatePropertyRequest;
#[allow(dead_code)]
pub type UpdateListingRequest = UpdatePropertyRequest;
