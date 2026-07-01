use crate::dtos::request::UpdateProfileRequest;
use crate::dtos::response::UserResponse;
use crate::models::user::User;
use crate::repository::user_repository::UserRepository;
use chrono::Utc;

use uuid::Uuid;

use rand::Rng;

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
        ref_code: Option<String>,
    ) -> Result<User, String> {
        // Validate referred_by_code
        let mut final_referred_by_code = None;
        if let Some(code) = ref_code {
            if self.user_repository.check_referral_code_exists(&code).await.unwrap_or(false) {
                final_referred_by_code = Some(code);
            }
        }

        let user_id = Uuid::new_v4();

        // Generate referral code (First 4 chars of UUID + 4 random digits)
        let uuid_str = user_id.to_string();
        let uuid_prefix = &uuid_str[..4].to_uppercase();
        let random_digits: String = (0..4).map(|_| {
            let mut rng = rand::thread_rng();
            rng.gen_range(0..10).to_string()
        }).collect();
        let generated_referral_code = format!("{}{}", uuid_prefix, random_digits);

        let user = User {
            id: user_id,
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
            referral_code: generated_referral_code,
            referred_by_code: final_referred_by_code.clone(),
        };

        match self.user_repository.create(user.clone()).await {
            Ok(_) => {
                // Insert into referrals table if there's a valid referred_by_code
                if let Some(referrer_code) = final_referred_by_code {
                    let _ = self.user_repository.insert_referral(&referrer_code, &user.id.to_string()).await;
                }
                Ok(user)
            },
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

    pub async fn process_referral_reward(&self, referred_user_id: &str) -> Result<(), String> {
        // Maximum 5 retries for coupon collision
        for _ in 0..5 {
            let coupon_code = {
                let mut rng = rand::thread_rng();
                let chars: String = (0..6)
                    .map(|_| {
                        let c = rng.gen_range(0..36);
                        if c < 10 {
                            (b'0' + c as u8) as char
                        } else {
                            (b'A' + (c - 10) as u8) as char
                        }
                    })
                    .collect();
                format!("EARN-{}", chars)
            };

            let amount = 1000;
            // E.g., expire in 6 months
            let expires_at = chrono::Utc::now().checked_add_signed(chrono::Duration::days(180));

            let result = self.user_repository.process_referral_reward(
                referred_user_id,
                &coupon_code,
                amount,
                expires_at
            ).await;

            match result {
                Ok(processed) => {
                    if processed {
                        log::info!("Successfully processed referral reward for user {}", referred_user_id);
                    }
                    return Ok(()); // Handled idempotently
                }
                Err(e) => {
                    // Check if it's a unique constraint violation for coupon_code
                    if e.contains("duplicate key value violates unique constraint") || e.contains("UNIQUE constraint failed") {
                        continue; // Retry
                    }
                    return Err(e);
                }
            }
        }
        let error_msg = "Failed to generate unique coupon code after 5 retries".to_string();
        log::error!("{} for user_id: {}", error_msg, referred_user_id);
        Err(error_msg)
    }
}
