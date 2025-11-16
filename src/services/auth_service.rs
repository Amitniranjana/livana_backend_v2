use crate::repository::user_repo::UserRepo;
use crate::models::user::User;
use crate::utils::auth::{hash_password, verify_password, create_jwt};
use sqlx::PgPool;
use anyhow::Result;

pub struct AuthService;

impl AuthService {
    pub async fn register(
        pool: &PgPool,
        first_name: String,
        last_name: String,
        email: String,
        password: String,
        phone_no: String,
        gender: String,
        user_role: String,
        business_name: Option<String>,
        license_number: Option<String>,
        experience_years: Option<i32>,
        commission_rate: Option<f64>,
        jwt_secret: &str,
    ) -> Result<(User, String)> {
        if UserRepo::find_by_email(pool, &email).await?.is_some() {
            anyhow::bail!("Email already exists");
        }

        let password_hash = hash_password(&password)?;
        let user = User::new(
            first_name,
            last_name,
            email.clone(),
            password_hash,
            phone_no,
            gender,
            user_role,
            business_name,
            license_number,
            experience_years,
            commission_rate,
        );

        let created = UserRepo::create(pool, &user).await?;
        let token = create_jwt(&created.id.to_string(), jwt_secret, 24)?;
        Ok((created, token))
    }

    pub async fn login(pool: &PgPool, email: String, password: String, jwt_secret: &str) -> Result<(User, String)> {
        let user = UserRepo::find_by_email(pool, &email).await?
            .ok_or_else(|| anyhow::anyhow!("Invalid credentials"))?;

        if !verify_password(&user.password_hash, &password) {
            anyhow::bail!("Invalid credentials");
        }

        let token = create_jwt(&user.id.to_string(), jwt_secret, 24)?;
        Ok((user, token))
    }
}
