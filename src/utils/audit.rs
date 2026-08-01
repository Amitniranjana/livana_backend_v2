use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub async fn log_audit_action(
    tx: &mut Transaction<'_, Postgres>,
    admin_email: &str,
    action: &str,
    entity_type: &str,
    entity_id: Option<Uuid>,
    reason: Option<&str>,
    metadata: Option<serde_json::Value>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO app_audit_logs (
            admin_email, action, entity_type, entity_id, reason, metadata
        ) VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        admin_email,
        action,
        entity_type,
        entity_id,
        reason,
        metadata
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}
