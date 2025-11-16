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
            phone_no
        ) VALUES ($1, $2, $3, $4, $5, $6)", );
         let result=   query.bind(&user.id)
            .bind(&user.first_name)
            .bind(&user.last_name)
            .bind(&user.email)
            .bind(&user.password)
            .bind(&user.phone_no).execute(&self.pool).await;

        match result {
            Ok(_)=>Ok(()),
            Err(e)=>{
                println!("{:?}",e);
                Err(e.to_string())
            },
        }

    }
}