/// CareCrew Tickets Repository
/// Raw sqlx::query() functions for ticket CRUD operations.
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;

// ─── Create ───────────────────────────────────────────────────────────────────

pub async fn create_ticket(
    db: &Pool<Postgres>,
    id: Uuid,
    user_id: Uuid,
    property_id: Option<Uuid>,
    issue_type: &str,
    description: &str,
    priority: &str,
) -> Result<sqlx::postgres::PgRow, sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO carecrew_tickets
            (id, user_id, property_id, issue_type, description, priority, status, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, 'OPEN', NOW(), NOW())
        RETURNING id, user_id, property_id, assignee_id, issue_type, description,
                  priority, status, created_at, updated_at
        "#
    )
    .bind(id)
    .bind(user_id)
    .bind(property_id)
    .bind(issue_type)
    .bind(description)
    .bind(priority)
    .fetch_one(db)
    .await
}

// ─── List (paginated) ─────────────────────────────────────────────────────────

pub async fn list_tickets_for_user(
    db: &Pool<Postgres>,
    user_id: Uuid,
    status_filter: Option<&str>,
    priority_filter: Option<&str>,
    page: i32,
    limit: i32,
) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
    let offset = ((page - 1) * limit) as i64;

    // Build WHERE clause dynamically
    match (status_filter, priority_filter) {
        (Some(s), Some(p)) => {
            sqlx::query(
                r#"SELECT id, user_id, property_id, assignee_id, issue_type, description,
                      priority, status, created_at, updated_at
               FROM carecrew_tickets
               WHERE user_id = $1 AND status = $2 AND priority = $3
               ORDER BY created_at DESC LIMIT $4 OFFSET $5"#,
            )
            .bind(user_id)
            .bind(s)
            .bind(p)
            .bind(limit as i64)
            .bind(offset)
            .fetch_all(db)
            .await
        }

        (Some(s), None) => {
            sqlx::query(
                r#"SELECT id, user_id, property_id, assignee_id, issue_type, description,
                      priority, status, created_at, updated_at
               FROM carecrew_tickets
               WHERE user_id = $1 AND status = $2
               ORDER BY created_at DESC LIMIT $3 OFFSET $4"#,
            )
            .bind(user_id)
            .bind(s)
            .bind(limit as i64)
            .bind(offset)
            .fetch_all(db)
            .await
        }

        (None, Some(p)) => {
            sqlx::query(
                r#"SELECT id, user_id, property_id, assignee_id, issue_type, description,
                      priority, status, created_at, updated_at
               FROM carecrew_tickets
               WHERE user_id = $1 AND priority = $2
               ORDER BY created_at DESC LIMIT $3 OFFSET $4"#,
            )
            .bind(user_id)
            .bind(p)
            .bind(limit as i64)
            .bind(offset)
            .fetch_all(db)
            .await
        }

        (None, None) => {
            sqlx::query(
                r#"SELECT id, user_id, property_id, assignee_id, issue_type, description,
                      priority, status, created_at, updated_at
               FROM carecrew_tickets
               WHERE user_id = $1
               ORDER BY created_at DESC LIMIT $2 OFFSET $3"#,
            )
            .bind(user_id)
            .bind(limit as i64)
            .bind(offset)
            .fetch_all(db)
            .await
        }
    }
}

pub async fn count_tickets_for_user(
    db: &Pool<Postgres>,
    user_id: Uuid,
    status_filter: Option<&str>,
    priority_filter: Option<&str>,
) -> Result<i64, sqlx::Error> {
    let row = match (status_filter, priority_filter) {
        (Some(s), Some(p)) => sqlx::query(
            "SELECT COUNT(*) as total FROM carecrew_tickets WHERE user_id=$1 AND status=$2 AND priority=$3"
        ).bind(user_id).bind(s).bind(p).fetch_one(db).await?,
        (Some(s), None) => sqlx::query(
            "SELECT COUNT(*) as total FROM carecrew_tickets WHERE user_id=$1 AND status=$2"
        ).bind(user_id).bind(s).fetch_one(db).await?,
        (None, Some(p)) => sqlx::query(
            "SELECT COUNT(*) as total FROM carecrew_tickets WHERE user_id=$1 AND priority=$2"
        ).bind(user_id).bind(p).fetch_one(db).await?,
        (None, None) => sqlx::query(
            "SELECT COUNT(*) as total FROM carecrew_tickets WHERE user_id=$1"
        ).bind(user_id).fetch_one(db).await?,
    };
    Ok(row.get::<i64, _>("total"))
}

// ─── Get single ticket ────────────────────────────────────────────────────────

pub async fn get_ticket_by_id(
    db: &Pool<Postgres>,
    ticket_id: Uuid,
) -> Result<Option<sqlx::postgres::PgRow>, sqlx::Error> {
    sqlx::query(
        r#"SELECT id, user_id, property_id, assignee_id, issue_type, description,
                  priority, status, created_at, updated_at
           FROM carecrew_tickets WHERE id = $1"#,
    )
    .bind(ticket_id)
    .fetch_optional(db)
    .await
}

// ─── Comments ─────────────────────────────────────────────────────────────────

pub async fn get_comments_for_ticket(
    db: &Pool<Postgres>,
    ticket_id: Uuid,
) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
    sqlx::query(
        r#"SELECT id, ticket_id, commenter_id, comment, created_at
           FROM carecrew_ticket_comments
           WHERE ticket_id = $1
           ORDER BY created_at ASC"#,
    )
    .bind(ticket_id)
    .fetch_all(db)
    .await
}

pub async fn add_comment(
    db: &Pool<Postgres>,
    id: Uuid,
    ticket_id: Uuid,
    commenter_id: Uuid,
    comment: &str,
) -> Result<sqlx::postgres::PgRow, sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO carecrew_ticket_comments (id, ticket_id, commenter_id, comment, created_at)
           VALUES ($1, $2, $3, $4, NOW())
           RETURNING id, ticket_id, commenter_id, comment, created_at"#,
    )
    .bind(id)
    .bind(ticket_id)
    .bind(commenter_id)
    .bind(comment)
    .fetch_one(db)
    .await
}

// ─── Update ───────────────────────────────────────────────────────────────────

pub async fn update_ticket(
    db: &Pool<Postgres>,
    ticket_id: Uuid,
    new_status: Option<&str>,
    assignee_id: Option<Uuid>,
) -> Result<sqlx::postgres::PgRow, sqlx::Error> {
    match (new_status, assignee_id) {
        (Some(s), Some(a)) => {
            sqlx::query(
                r#"UPDATE carecrew_tickets SET status=$1, assignee_id=$2, updated_at=NOW()
               WHERE id=$3
               RETURNING id, user_id, property_id, assignee_id, issue_type, description,
                         priority, status, created_at, updated_at"#,
            )
            .bind(s)
            .bind(a)
            .bind(ticket_id)
            .fetch_one(db)
            .await
        }

        (Some(s), None) => {
            sqlx::query(
                r#"UPDATE carecrew_tickets SET status=$1, updated_at=NOW()
               WHERE id=$2
               RETURNING id, user_id, property_id, assignee_id, issue_type, description,
                         priority, status, created_at, updated_at"#,
            )
            .bind(s)
            .bind(ticket_id)
            .fetch_one(db)
            .await
        }

        (None, Some(a)) => {
            sqlx::query(
                r#"UPDATE carecrew_tickets SET assignee_id=$1, updated_at=NOW()
               WHERE id=$2
               RETURNING id, user_id, property_id, assignee_id, issue_type, description,
                         priority, status, created_at, updated_at"#,
            )
            .bind(a)
            .bind(ticket_id)
            .fetch_one(db)
            .await
        }

        (None, None) => {
            // Nothing to update — just return current state
            sqlx::query(
                r#"SELECT id, user_id, property_id, assignee_id, issue_type, description,
                          priority, status, created_at, updated_at
                   FROM carecrew_tickets WHERE id=$1"#,
            )
            .bind(ticket_id)
            .fetch_one(db)
            .await
        }
    }
}
