use crate::dtos::request::UpdateProfileRequest;
use crate::dtos::response::UserResponse;
use crate::models::user::User;
use crate::repository::user_repository::UserRepository;
use chrono::Utc;

use uuid::Uuid;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct UserService {
    pub user_repository: UserRepository,
}

impl UserService {
    pub fn new(user_repository: UserRepository) -> Self {
        UserService { user_repository }

    }

    pub async fn create_user(
        &self,
        first_name: &str,
        last_name: &str,
        email: &str,
        phone_no: &str,
        password: &str,
        _gender: &str,
        user_role: &str,
    ) -> Result<User, String> {
        let user = User {
            id: Uuid::new_v4(),
            first_name: first_name.to_string(),
            phone_no: phone_no.to_string(),
            last_name: last_name.to_string(),
            email: email.to_string(),
            password: password.to_string(),
            user_role: user_role.to_string(),
            verified: false,
            last_active: Some(Utc::now()),
            status: "active".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            chime_user_arn: None,
            google_id: None,
            profile_picture: None,
            associate_type: None,
            is_phone_verified: false,
        };

        match self.user_repository.create(user.clone()).await {
            Ok(_) => Ok(user),
            Err(e) => Err(e),
        }
    }

    pub async fn find_by_email(&self, email: &str) -> Result<Option<User>, String> {
        self.user_repository.find_by_email(email).await
    }



    pub async fn update_password(
        &self,
        user_id: &str,
        new_password_hash: &str,
    ) -> Result<(), String> {
        self.user_repository
            .update_password(user_id, new_password_hash)
            .await
    }



    pub async fn get_user_profile(&self, user_id: &str) -> Result<UserResponse, String> {
        // 1. Get basic user info
        let user = self
            .user_repository
            .find_by_id(user_id)
            .await?
            .ok_or_else(|| "User not found".to_string())?;

        // 2. Get extended profile info (from user_profiles)
        // Ignoring error if table doesn't exist or query fails (optional)
        // But better to handle it.
        let extended = self
            .user_repository
            .get_extended_profile(user_id)
            .await
            .unwrap_or(None);
        let (gender, bio, profile_image_url) = extended.unwrap_or((None, None, None));

        // 3. Construct response
        // Note: Broker fields are left as None as they should be fetched via broker API or we could fetch them here if needed.
        Ok(UserResponse {
            id: user.id.to_string(),
            first_name: user.first_name,
            last_name: user.last_name,
            email: user.email,
            phone_no: user.phone_no,
            gender: gender.unwrap_or_default(), // or from User struct if added there
            user_role: user.user_role,
            verified: user.verified,
            profile_image_url,
            bio,
            business_name: None,
            license_number: None,
            experience_years: None,
            commission_rate: None,
            broker_rating: None,
            total_reviews: None,
            is_verified_broker: false, // This logic probably needs specific check if needed
            status: user.status,
            created_at: user.created_at.to_rfc3339(),
        })
    }

    pub async fn update_user_profile(
        &self,
        user_id: &str,
        req: UpdateProfileRequest,
    ) -> Result<UserResponse, String> {
        // 1. Update basic user info
        self.user_repository
            .update_user(user_id, req.first_name, req.last_name, req.phone_no)
            .await?;

        // 2. Update extended profile info
        self.user_repository
            .upsert_profile(user_id, req.gender, req.bio, None)
            .await?;

        // 3. Return updated profile
        self.get_user_profile(user_id).await
    }

    pub async fn update_profile_image(
        &self,
        user_id: &str,
        image_url: &str,
    ) -> Result<UserResponse, String> {
        // Update only image url
        self.user_repository
            .upsert_profile(user_id, None, None, Some(image_url.to_string()))
            .await?;
        self.get_user_profile(user_id).await
    }

    pub async fn update_chime_arn(&self, user_id: &str, arn: &str) -> Result<(), String> {
        self.user_repository
            .update_chime_user_arn(user_id, arn)
            .await
    }

    // -------------------------------------------------------------------------
    // Associate flow methods
    // -------------------------------------------------------------------------

    pub async fn find_by_phone(&self, phone: &str) -> Result<Option<User>, String> {
        self.user_repository.find_by_phone(phone).await
    }

    pub async fn set_phone_verified(&self, user_id: &str) -> Result<(), String> {
        self.user_repository.set_phone_verified(user_id).await
    }

    pub async fn update_associate_type(
        &self,
        user_id: &str,
        associate_type: &str,
    ) -> Result<(), String> {
        self.user_repository
            .update_associate_type(user_id, associate_type)
            .await
    }

    pub async fn store_otp(
        &self,
        phone_no: &str,
        otp_code: &str,
        expires_minutes: i64,
    ) -> Result<(), String> {
        self.user_repository
            .store_otp(phone_no, otp_code, expires_minutes)
            .await
    }

    pub async fn verify_and_consume_otp(
        &self,
        phone_no: &str,
        otp_code: &str,
    ) -> Result<bool, String> {
        self.user_repository.verify_otp(phone_no, otp_code).await
    }

    pub async fn invalidate_otps(&self, phone_no: &str) -> Result<(), String> {
        self.user_repository.invalidate_otps(phone_no).await
    }

    // -------------------------------------------------------------------------
    // Email-based OTP methods
    // -------------------------------------------------------------------------

    /// Store an OTP for an email address with a given purpose.
    pub async fn store_email_otp(
        &self,
        email: &str,
        otp_code: &str,
        purpose: &str,
        expires_minutes: i64,
    ) -> Result<(), String> {
        self.user_repository
            .store_email_otp(email, otp_code, purpose, expires_minutes)
            .await
    }

    /// Verify and consume an email OTP for a given purpose.
    pub async fn verify_and_consume_email_otp(
        &self,
        email: &str,
        otp_code: &str,
        purpose: &str,
    ) -> Result<bool, String> {
        self.user_repository
            .verify_email_otp(email, otp_code, purpose)
            .await
    }

    /// Invalidate all unused OTPs for an email + purpose.
    pub async fn invalidate_email_otps(&self, email: &str, purpose: &str) -> Result<(), String> {
        self.user_repository
            .invalidate_email_otps(email, purpose)
            .await
    }
}
