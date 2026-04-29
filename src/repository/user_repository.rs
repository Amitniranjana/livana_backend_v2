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
            associate_type
        ) VALUES ($1::uuid, $2, $3, $4, $5, $6, $7, $8)",
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
}
