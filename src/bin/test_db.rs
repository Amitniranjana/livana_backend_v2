use sqlx::PgPool;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    let pool = PgPool::connect("postgresql://postgres:password1235@localhost:5433/livana_db")
        .await
        .unwrap();
    let rows: Vec<(Uuid,)> = sqlx::query_as("SELECT id FROM users LIMIT 3")
        .fetch_all(&pool)
        .await
        .unwrap();
    for row in rows {
        println!("USER ID: {}", row.0);
    }
}
