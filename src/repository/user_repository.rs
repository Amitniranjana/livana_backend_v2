use sqlx::PgPool;
use crate::models::user::User;
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct UserRepository{
    pub pool: PgPool,
}
#[allow(dead_code)]
impl UserRepository{
    pub fn new(pg_pool: PgPool)->Self{
        UserRepository{
            pool: pg_pool
        }
    }

    // to find the email

pub async fn find_by_email(&self, email: &str) -> Result<Option<User>, String> { // <-- Note: Changed signature
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
            user_role
        ) VALUES ($1::uuid, $2, $3, $4, $5, $6,$7)", );
         let result=   query.bind(&user.id)
            .bind(&user.first_name)
            .bind(&user.last_name)
            .bind(&user.email)
            .bind(&user.password)

            .bind(&user.phone_no)
            .bind(&user.user_role)
            .execute(&self.pool).await;

        match result {
            Ok(_)=>Ok(()),
            Err(e)=>{
                println!("{:?}",e);
                Err(e.to_string())
            },
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

    pub async fn update_user(&self, id: &str, first_name: Option<String>, last_name: Option<String>, phone_no: Option<String>) -> Result<(), String> {
        let uuid = uuid::Uuid::parse_str(id).map_err(|e| e.to_string())?;
        // Simple dynamic query construction or just use COALESCE if always passing existing values?
        // Since we pass Option, it's better to verify what to update.
        // For simplicity, let's use a query that updates provided non-None values.
        // But separate queries are easier or builder.
        // Let's assume we update all 3 if provided, keeping old if None?
        // Actually COALESCE logic is handled in Service or here.
        // Let's do: update users set first_name = COALESCE($2, first_name), etc.

        let result = sqlx::query(
            "UPDATE users SET
             first_name = COALESCE($2, first_name),
             last_name = COALESCE($3, last_name),
             phone_no = COALESCE($4, phone_no),
             updated_at = NOW()
             WHERE id = $1"
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

    // Upsert into user_profiles
    // Note: This assumes a table `user_profiles` exists.
    pub async fn upsert_profile(&self, user_id: &str, gender: Option<String>, bio: Option<String>, profile_image_url: Option<String>) -> Result<(), String> {
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

    pub async fn get_extended_profile(&self, user_id: &str) -> Result<Option<(Option<String>, Option<String>, Option<String>)>, String> {
        // Returns (gender, bio, profile_image_url)
        let uuid = uuid::Uuid::parse_str(user_id).map_err(|e| e.to_string())?;

        struct ProfileRow {
            gender: Option<String>,
            bio: Option<String>,
            profile_image_url: Option<String>,
        }

        let result = sqlx::query_as!(
            ProfileRow,
            "SELECT gender, bio, profile_image_url FROM user_profiles WHERE user_id = $1",
            uuid
        )
        .fetch_optional(&self.pool)
        .await;

        match result {
            Ok(Some(row)) => Ok(Some((row.gender, row.bio, row.profile_image_url))),
            Ok(None) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
}