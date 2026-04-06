// src/utils/notification_chat_helper.rs
//
// Reusable helper functions for creating notifications and chats.
// Called by vibes.rs (send_vibe) and visit.rs (book_visit_handler)
// after their primary DB operations succeed.

use sqlx::{Pool, Postgres};
use uuid::Uuid;

/// Create a chat between two users if one doesn't already exist.
/// Inserts an initial message into the chat.
/// Returns the chat_id (existing or newly created).
pub async fn create_chat_if_not_exists(
    db: &Pool<Postgres>,
    user_a: Uuid,
    user_b: Uuid,
    initial_message: &str,
) -> Result<Uuid, String> {
    // 1. Check if a chat already exists between user_a and user_b
    let existing_chat: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT cp1.chat_id
        FROM chat_participants cp1
        JOIN chat_participants cp2 ON cp1.chat_id = cp2.chat_id
        JOIN chats c ON c.id = cp1.chat_id
        WHERE cp1.user_id = $1
          AND cp2.user_id = $2
          AND c.is_deleted = FALSE
        LIMIT 1
        "#,
    )
    .bind(user_a)
    .bind(user_b)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Failed to check existing chat: {}", e))?;

    let chat_id = if let Some((id,)) = existing_chat {
        id
    } else {
        // 2. Create a new chat
        let new_chat_id = Uuid::new_v4();
        sqlx::query("INSERT INTO chats (id, created_at) VALUES ($1, NOW())")
            .bind(new_chat_id)
            .execute(db)
            .await
            .map_err(|e| format!("Failed to create chat: {}", e))?;

        // 3. Add both users as participants
        sqlx::query(
            "INSERT INTO chat_participants (chat_id, user_id, joined_at) VALUES ($1, $2, NOW())",
        )
        .bind(new_chat_id)
        .bind(user_a)
        .execute(db)
        .await
        .map_err(|e| format!("Failed to add participant A: {}", e))?;

        sqlx::query(
            "INSERT INTO chat_participants (chat_id, user_id, joined_at) VALUES ($1, $2, NOW())",
        )
        .bind(new_chat_id)
        .bind(user_b)
        .execute(db)
        .await
        .map_err(|e| format!("Failed to add participant B: {}", e))?;

        new_chat_id
    };

    // 4. Insert the initial message (sender is user_a)
    let message_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO messages (id, chat_id, sender_id, content, created_at) VALUES ($1, $2, $3, $4, NOW())",
    )
    .bind(message_id)
    .bind(chat_id)
    .bind(user_a)
    .bind(initial_message)
    .execute(db)
    .await
    .map_err(|e| format!("Failed to insert initial message: {}", e))?;

    println!(
        "[ChatHelper] Chat {} between {} and {} — message inserted",
        chat_id, user_a, user_b
    );

    Ok(chat_id)
}

/// Create a notification for a user.
/// Returns the notification_id.
pub async fn create_notification(
    db: &Pool<Postgres>,
    user_id: Uuid,
    title: &str,
    body: &str,
    notification_type: &str,
    related_entity_id: Option<Uuid>,
    related_entity_type: Option<&str>,
) -> Result<Uuid, String> {
    let notification_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO notifications (id, user_id, title, message, type, is_read, related_entity_id, related_entity_type, created_at)
        VALUES ($1, $2, $3, $4, $5, FALSE, $6, $7, NOW())
        "#,
    )
    .bind(notification_id)
    .bind(user_id)
    .bind(title)
    .bind(body)
    .bind(notification_type)
    .bind(related_entity_id)
    .bind(related_entity_type)
    .execute(db)
    .await
    .map_err(|e| format!("Failed to create notification: {}", e))?;

    println!(
        "[NotificationHelper] Notification {} created for user {} — type={}",
        notification_id, user_id, notification_type
    );

    Ok(notification_id)
}

/// Fetch a user's display name (first_name + last_name) from the users table.
pub async fn get_user_display_name(db: &Pool<Postgres>, user_id: Uuid) -> Result<String, String> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT first_name, last_name FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(db)
            .await
            .map_err(|e| format!("Failed to fetch user name: {}", e))?;

    match row {
        Some((first, last)) => Ok(format!("{} {}", first, last).trim().to_string()),
        None => Ok("A user".to_string()),
    }
}
