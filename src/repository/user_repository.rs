use crate::models::user::User;
use sqlx::PgPool;
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct UserRepository {
    pub pool: PgPool,
}
#[allow(dead_code)]
impl UserRepository {
    pub fn new(pg_pool: PgPool) -> Self {
        UserRepository { pool: pg_pool }
    }

    // to find the email

    pub async fn find_by_email(&self, email: &str) -> Result<Option<User>, String> {
        // <-- Note: Changed signature
        let result = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
            .bind(email)
            .fetch_optional(&self.pool) // <-- Use self.pool
            .await;

        match result {
            Ok(user) => Ok(user),
            Err(e) => Err(e.to_string()), // <-- Handle the error
        }
    }

    // only creates a user
    pub async fn create(&self, user: User) -> Result<(), String> {
        let query = sqlx::query(
            "INSERT INTO users (
            id,
            first_name,
            last_name,
            email,
            password,
            phone_no,
            user_role,
            associate_type,
            referral_code,
            referred_by_code
        ) VALUES ($1::uuid, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        );
        let result = query
            .bind(&user.id)
            .bind(&user.first_name)
            .bind(&user.last_name)
            .bind(&user.email)
            .bind(&user.password)
            .bind(&user.phone_no)
            .bind(&user.user_role)
            .bind(None::<String>)
            .bind(&user.referral_code)
            .bind(&user.referred_by_code)
            .execute(&self.pool)
            .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                println!("{:?}", e);
                Err(e.to_string())
            }
        }
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<User>, String> {
        let uuid = uuid::Uuid::parse_str(id).map_err(|e| e.to_string())?;
        let result = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(uuid)
            .fetch_optional(&self.pool)
            .await;

        match result {
            Ok(user) => Ok(user),
            Err(e) => Err(e.to_string()),
        }
    }

    pub async fn update_user(
        &self,
        id: &str,
        first_name: Option<String>,
        last_name: Option<String>,
        phone_no: Option<String>,
    ) -> Result<(), String> {
        let uuid = uuid::Uuid::parse_str(id).map_err(|e| e.to_string())?;

        let result = sqlx::query(
            "UPDATE users SET
             first_name = COALESCE($2, first_name),
             last_name = COALESCE($3, last_name),
             phone_no = COALESCE($4, phone_no),
             updated_at = NOW()
             WHERE id = $1",
        )
        .bind(uuid)
        .bind(first_name)
        .bind(last_name)
        .bind(phone_no)
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    pub async fn update_chime_user_arn(&self, user_id: &str, arn: &str) -> Result<(), String> {
        let uuid = uuid::Uuid::parse_str(user_id).map_err(|e| e.to_string())?;
        let result = sqlx::query("UPDATE users SET chime_user_arn = $2 WHERE id = $1")
            .bind(uuid)
            .bind(arn)
            .execute(&self.pool)
            .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    // Upsert into user_profiles
    // Note: This assumes a table `user_profiles` exists.
    pub async fn upsert_profile(
        &self,
        user_id: &str,
        gender: Option<String>,
        bio: Option<String>,
        profile_image_url: Option<String>,
    ) -> Result<(), String> {
        let uuid = uuid::Uuid::parse_str(user_id).map_err(|e| e.to_string())?;

        let result = sqlx::query(
            "INSERT INTO user_profiles (user_id, gender, bio, profile_image_url)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (user_id) DO UPDATE
             SET gender = COALESCE(EXCLUDED.gender, user_profiles.gender),
                 bio = COALESCE(EXCLUDED.bio, user_profiles.bio),
                 profile_image_url = COALESCE(EXCLUDED.profile_image_url, user_profiles.profile_image_url)"
        )
        .bind(uuid)
        .bind(gender)
        .bind(bio)
        .bind(profile_image_url)
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    pub async fn get_extended_profile(
        &self,
        user_id: &str,
    ) -> Result<Option<(Option<String>, Option<String>, Option<String>)>, String> {
        // Returns (gender, bio, profile_image_url)
        let uuid = uuid::Uuid::parse_str(user_id).map_err(|e| e.to_string())?;

        #[derive(sqlx::FromRow)]
        struct ProfileRow {
            gender: Option<String>,
            bio: Option<String>,
            profile_image_url: Option<String>,
        }

        let result = sqlx::query_as::<_, ProfileRow>(
            "SELECT gender, bio, profile_image_url FROM user_profiles WHERE user_id = $1",
        )
        .bind(uuid)
        .fetch_optional(&self.pool)
        .await;

        match result {
            Ok(Some(row)) => Ok(Some((row.gender, row.bio, row.profile_image_url))),
            Ok(None) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    // -------------------------------------------------------------------------
    // Google OAuth methods
    // -------------------------------------------------------------------------

    /// Find a user by their stable Google subject ID (`sub` field).
    pub async fn find_by_google_id(&self, google_id: &str) -> Result<Option<User>, String> {
        let result = sqlx::query_as::<_, User>("SELECT * FROM users WHERE google_id = $1")
            .bind(google_id)
            .fetch_optional(&self.pool)
            .await;

        match result {
            Ok(user) => Ok(user),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Upsert a Google-authenticated user.  Safe to call on every sign-in:
    /// returning users have their name/picture refreshed, new users are inserted.
    /// `phone_no` and `password` are stored as empty strings to satisfy the
    /// NOT NULL constraint (Google auth doesn't provide these).
    pub async fn create_google_user(
        &self,
        google_id: &str,
        email: &str,
        name: &str,
        picture: Option<&str>,
    ) -> Result<User, String> {
        let id = uuid::Uuid::new_v4();
        // Best-effort split of a display name into first / last
        let parts: Vec<&str> = name.splitn(2, ' ').collect();
        let first_name = parts.first().copied().unwrap_or(name);
        let last_name = parts.get(1).copied().unwrap_or("");

        let result = sqlx::query_as::<_, User>(
            r#"INSERT INTO users
                   (id, google_id, email, first_name, last_name, phone_no, password,
                    profile_picture, user_role, verified, status)
               VALUES ($1, $2, $3, $4, $5, '', '', $6, 'user', TRUE, 'active')
               ON CONFLICT (google_id) DO UPDATE
                   SET email           = EXCLUDED.email,
                       first_name      = EXCLUDED.first_name,
                       last_name       = EXCLUDED.last_name,
                       profile_picture = EXCLUDED.profile_picture,
                       updated_at      = NOW()
               RETURNING *"#,
        )
        .bind(id)
        .bind(google_id)
        .bind(email)
        .bind(first_name)
        .bind(last_name)
        .bind(picture)
        .fetch_one(&self.pool)
        .await;

        match result {
            Ok(user) => Ok(user),
            Err(e) => Err(e.to_string()),
        }
    }

    // -------------------------------------------------------------------------
    // Associate flow methods
    // -------------------------------------------------------------------------

    /// Find a user by phone number (for phone-based login).
    pub async fn find_by_phone(&self, phone: &str) -> Result<Option<User>, String> {
        let result = sqlx::query_as::<_, User>("SELECT * FROM users WHERE phone_no = $1")
            .bind(phone)
            .fetch_optional(&self.pool)
            .await;

        match result {
            Ok(user) => Ok(user),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Mark a user's phone as verified.
    pub async fn set_phone_verified(&self, user_id: &str) -> Result<(), String> {
        let uuid = uuid::Uuid::parse_str(user_id).map_err(|e| e.to_string())?;
        let result = sqlx::query(
            "UPDATE users SET is_phone_verified = TRUE, updated_at = NOW() WHERE id = $1",
        )
        .bind(uuid)
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Update the associate_type for a user.
    pub async fn update_associate_type(
        &self,
        user_id: &str,
        associate_type: &str,
    ) -> Result<(), String> {
        let uuid = uuid::Uuid::parse_str(user_id).map_err(|e| e.to_string())?;
        let result =
            sqlx::query("UPDATE users SET associate_type = $2, updated_at = NOW() WHERE id = $1")
                .bind(uuid)
                .bind(associate_type)
                .execute(&self.pool)
                .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Store an OTP for a phone number. Invalidates any existing unused OTPs first.
    pub async fn store_otp(
        &self,
        phone_no: &str,
        otp_code: &str,
        expires_minutes: i64,
    ) -> Result<(), String> {
        // Invalidate existing OTPs for this phone
        let _ =
            sqlx::query("UPDATE otp_records SET used = TRUE WHERE phone_no = $1 AND used = FALSE")
                .bind(phone_no)
                .execute(&self.pool)
                .await;

        let id = uuid::Uuid::new_v4();
        let expires_at = chrono::Utc::now() + chrono::Duration::minutes(expires_minutes);

        let result = sqlx::query(
            "INSERT INTO otp_records (id, phone_no, otp_code, expires_at) VALUES ($1, $2, $3, $4)",
        )
        .bind(id)
        .bind(phone_no)
        .bind(otp_code)
        .bind(expires_at)
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Verify an OTP. Returns:
    /// - Ok(true) if valid and consumed
    /// - Ok(false) if wrong code
    /// - Err("OTP has expired") if expired
    /// - Err(...) on DB error
    pub async fn verify_otp(&self, phone_no: &str, otp_code: &str) -> Result<bool, String> {
        // Find the latest unused OTP for this phone
        #[derive(sqlx::FromRow)]
        struct OtpRow {
            id: uuid::Uuid,
            otp_code: String,
            expires_at: chrono::DateTime<chrono::Utc>,
        }

        let result = sqlx::query_as::<_, OtpRow>(
            r#"SELECT id, otp_code, expires_at
               FROM otp_records
               WHERE phone_no = $1 AND used = FALSE
               ORDER BY created_at DESC
               LIMIT 1"#,
        )
        .bind(phone_no)
        .fetch_optional(&self.pool)
        .await;

        match result {
            Ok(Some(row)) => {
                // Check expiry
                if chrono::Utc::now() > row.expires_at {
                    // Mark as used so it can't be retried
                    let _ = sqlx::query("UPDATE otp_records SET used = TRUE WHERE id = $1")
                        .bind(row.id)
                        .execute(&self.pool)
                        .await;
                    return Err("OTP has expired".to_string());
                }
                // Check code
                if row.otp_code == otp_code {
                    // Consume it
                    let _ = sqlx::query("UPDATE otp_records SET used = TRUE WHERE id = $1")
                        .bind(row.id)
                        .execute(&self.pool)
                        .await;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Ok(None) => Err("No OTP found for this phone number".to_string()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Invalidate all unused OTPs for a phone number.
    pub async fn invalidate_otps(&self, phone_no: &str) -> Result<(), String> {
        let result =
            sqlx::query("UPDATE otp_records SET used = TRUE WHERE phone_no = $1 AND used = FALSE")
                .bind(phone_no)
                .execute(&self.pool)
                .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Update a user's password hash in the database.
    pub async fn update_password(
        &self,
        user_id: &str,
        new_password_hash: &str,
    ) -> Result<(), String> {
        let uuid = uuid::Uuid::parse_str(user_id).map_err(|e| e.to_string())?;
        let result =
            sqlx::query("UPDATE users SET password = $2, updated_at = NOW() WHERE id = $1")
                .bind(uuid)
                .bind(new_password_hash)
                .execute(&self.pool)
                .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    // -------------------------------------------------------------------------
    // Email-based OTP methods
    // -------------------------------------------------------------------------

    /// Store an OTP for an email address with a specific purpose.
    /// Invalidates any existing unused OTPs for the same email + purpose first.
    pub async fn store_email_otp(
        &self,
        email: &str,
        otp_code: &str,
        purpose: &str,
        expires_minutes: i64,
    ) -> Result<(), String> {
        // Invalidate existing OTPs for this email + purpose
        let _ = sqlx::query(
            "UPDATE otp_records SET used = TRUE WHERE email = $1 AND purpose = $2 AND used = FALSE",
        )
        .bind(email)
        .bind(purpose)
        .execute(&self.pool)
        .await;

        let id = uuid::Uuid::new_v4();
        let expires_at = chrono::Utc::now() + chrono::Duration::minutes(expires_minutes);

        let result = sqlx::query(
            "INSERT INTO otp_records (id, email, otp_code, purpose, expires_at) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(id)
        .bind(email)
        .bind(otp_code)
        .bind(purpose)
        .bind(expires_at)
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Verify an email OTP for a given purpose. Returns:
    /// - `Ok(true)` if valid and consumed
    /// - `Ok(false)` if wrong code
    /// - `Err("OTP has expired")` if expired
    /// - `Err("No OTP found ...")` if none exists
    pub async fn verify_email_otp(
        &self,
        email: &str,
        otp_code: &str,
        purpose: &str,
    ) -> Result<bool, String> {
        #[derive(sqlx::FromRow)]
        struct OtpRow {
            id: uuid::Uuid,
            otp_code: String,
            expires_at: chrono::DateTime<chrono::Utc>,
        }

        let result = sqlx::query_as::<_, OtpRow>(
            r#"SELECT id, otp_code, expires_at
               FROM otp_records
               WHERE email = $1 AND purpose = $2 AND used = FALSE
               ORDER BY created_at DESC
               LIMIT 1"#,
        )
        .bind(email)
        .bind(purpose)
        .fetch_optional(&self.pool)
        .await;

        match result {
            Ok(Some(row)) => {
                // Check expiry
                if chrono::Utc::now() > row.expires_at {
                    let _ = sqlx::query("UPDATE otp_records SET used = TRUE WHERE id = $1")
                        .bind(row.id)
                        .execute(&self.pool)
                        .await;
                    return Err("OTP has expired".to_string());
                }
                // Check code
                if row.otp_code == otp_code {
                    let _ = sqlx::query("UPDATE otp_records SET used = TRUE WHERE id = $1")
                        .bind(row.id)
                        .execute(&self.pool)
                        .await;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Ok(None) => Err("No OTP found for this email".to_string()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Invalidate all unused OTPs for an email + purpose combination.
    pub async fn invalidate_email_otps(&self, email: &str, purpose: &str) -> Result<(), String> {
        let result = sqlx::query(
            "UPDATE otp_records SET used = TRUE WHERE email = $1 AND purpose = $2 AND used = FALSE",
        )
        .bind(email)
        .bind(purpose)
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    // -------------------------------------------------------------------------
    // Referral methods
    // -------------------------------------------------------------------------

    /// Check if a referral code exists and belongs to a user
    pub async fn check_referral_code_exists(&self, code: &str) -> Result<bool, String> {
        let result = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE referral_code = $1")
            .bind(code)
            .fetch_one(&self.pool)
            .await;

        match result {
            Ok(count) => Ok(count > 0),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Insert a new referral record
    pub async fn insert_referral(&self, referrer_code: &str, referred_user_id: &str) -> Result<(), String> {
        // 1. Get the referrer's user ID
        let referrer = sqlx::query_as::<_, User>("SELECT * FROM users WHERE referral_code = $1")
            .bind(referrer_code)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let referrer_user = match referrer {
            Some(u) => u,
            None => return Ok(()), // Should not happen if validated, but safe fallback
        };

        let referred_uuid = uuid::Uuid::parse_str(referred_user_id).map_err(|e| e.to_string())?;

        // Block self referral just in case
        if referrer_user.id == referred_uuid {
            return Ok(());
        }

        let referral_id = uuid::Uuid::new_v4();
        let reward_amount = 1000; // As per requirements

        let result = sqlx::query(
            "INSERT INTO referrals (id, referrer_user_id, referred_user_id, status, reward_amount) 
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(referral_id)
        .bind(referrer_user.id)
        .bind(referred_uuid)
        .bind("pending")
        .bind(reward_amount)
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Get referral statistics for a user
    pub async fn get_referral_stats(&self, user_id: &str) -> Result<(i64, i64, i64), String> {
        let uuid = uuid::Uuid::parse_str(user_id).map_err(|e| e.to_string())?;
        
        #[derive(sqlx::FromRow)]
        struct StatsRow {
            total_referrals: Option<i64>,
            total_rewards_earned: Option<i64>,
            pending_referrals: Option<i64>,
        }

        let result = sqlx::query_as::<_, StatsRow>(
            r#"SELECT 
                 COUNT(CASE WHEN status IN ('completed', 'rewarded') THEN 1 END) as total_referrals,
                 SUM(CASE WHEN status IN ('completed', 'rewarded') THEN reward_amount ELSE 0 END) as total_rewards_earned,
                 COUNT(CASE WHEN status = 'pending' THEN 1 END) as pending_referrals
               FROM referrals 
               WHERE referrer_user_id = $1"#,
        )
        .bind(uuid)
        .fetch_one(&self.pool)
        .await;

        match result {
            Ok(row) => Ok((
                row.total_referrals.unwrap_or(0),
                row.total_rewards_earned.unwrap_or(0),
                row.pending_referrals.unwrap_or(0)
            )),
            Err(e) => Err(e.to_string()),
        }
    }

    pub async fn process_referral_reward(
        &self,
        referred_user_id: &str,
        coupon_code: &str,
        amount: i32,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<bool, String> {
        let referred_uuid = match uuid::Uuid::parse_str(referred_user_id) {
            Ok(id) => id,
            Err(e) => return Err(e.to_string()),
        };

        // 1. Find pending referral
        #[derive(sqlx::FromRow)]
        struct PendingReferral {
            id: uuid::Uuid,
            referrer_user_id: uuid::Uuid,
        }

        let referral = sqlx::query_as::<_, PendingReferral>(
            "SELECT id, referrer_user_id FROM referrals WHERE referred_user_id = $1 AND status = 'pending'"
        )
        .bind(referred_uuid)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        let referral = match referral {
            Some(r) => r,
            None => return Ok(false), // No pending referral found, idempotent success
        };

        // 2. Update to completed
        sqlx::query("UPDATE referrals SET status = 'completed', completed_at = NOW() WHERE id = $1")
            .bind(referral.id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        // 3. Insert into referral_rewards
        let reward_id = uuid::Uuid::new_v4();
        sqlx::query(
            "INSERT INTO referral_rewards (id, user_id, referral_id, coupon_code, amount, status, expires_at) 
             VALUES ($1, $2, $3, $4, $5, 'active', $6)"
        )
        .bind(reward_id)
        .bind(referral.referrer_user_id)
        .bind(referral.id)
        .bind(coupon_code)
        .bind(amount)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        // 4. Update to rewarded
        sqlx::query("UPDATE referrals SET status = 'rewarded', rewarded_at = NOW() WHERE id = $1")
            .bind(referral.id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        Ok(true)
    }

    pub async fn get_referral_rewards(&self, user_id: &str) -> Result<Vec<crate::dtos::response::RewardItemData>, String> {
        let user_uuid = uuid::Uuid::parse_str(user_id).map_err(|e| e.to_string())?;

        #[derive(sqlx::FromRow)]
        struct RewardItemRow {
            id: uuid::Uuid,
            coupon_code: String,
            amount: i32,
            status: String,
            referred_user_name: String,
            created_at: chrono::DateTime<chrono::Utc>,
            expires_at: Option<chrono::DateTime<chrono::Utc>>,
        }
        
        let rows = sqlx::query_as::<_, RewardItemRow>(
            r#"
            SELECT 
                rw.id, 
                rw.coupon_code, 
                rw.amount, 
                rw.status, 
                u.first_name || ' ' || SUBSTRING(u.last_name FROM 1 FOR 1) || '.' as referred_user_name,
                rw.created_at, 
                rw.expires_at
            FROM referral_rewards rw
            JOIN referrals r ON rw.referral_id = r.id
            JOIN users u ON r.referred_user_id = u.id
            WHERE rw.user_id = $1
            ORDER BY rw.created_at DESC
            "#
        )
        .bind(user_uuid)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        let rewards = rows.into_iter().map(|row| crate::dtos::response::RewardItemData {
            id: row.id,
            coupon_code: row.coupon_code,
            amount: row.amount,
            status: row.status,
            referred_user_name: row.referred_user_name,
            created_at: row.created_at,
            expires_at: row.expires_at,
        }).collect();

        Ok(rewards)
    }
}
