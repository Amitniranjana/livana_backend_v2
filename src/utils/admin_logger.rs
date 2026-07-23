use sqlx::PgPool;
use uuid::Uuid;
use serde_json::Value;

pub async fn log_admin_action(
    db: &PgPool,
    admin_id: &str,
    action_type: &str,
    target_type: &str,
    target_id: Option<Uuid>,
    details: Option<Value>,
) -> Result<(), sqlx::Error> {
    let log_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO admin_action_logs (id, admin_id, action_type, target_type, target_id, details)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#
    )
    .bind(log_id)
    .bind(admin_id)
    .bind(action_type)
    .bind(target_type)
    .bind(target_id)
    .bind(details)
    .execute(db)
    .await?;

    Ok(())
}
