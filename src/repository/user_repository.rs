use sqlx::PgPool;
use crate::models::user::User;
#[derive(Clone, Debug)]
pub struct UserRepository{
    pub pool: PgPool,
}

impl UserRepository{
    pub fn new(pg_pool: PgPool)->Self{
        UserRepository{
            pool: pg_pool
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