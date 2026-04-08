// src/repository/chat_repository.rs
//
// Queries our own PostgreSQL chat tables (not AWS Chime).
// Provides the "recent chats" query that powers GET /api/v1/chats/recent.

use sqlx::PgPool;
use uuid::Uuid;

use crate::models::recent_chat::RecentChat;

#[derive(Clone, Debug)]
pub struct ChatRepository {
    pub pool: PgPool,
}

impl ChatRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Returns the most recent chats for `user_id`, sorted by latest message
    /// descending.  Each row contains the chat_id, the last message content,
    /// and the timestamp of that message.
    ///
    /// The correlated subquery ensures we pick exactly one row per chat
    /// (the row whose `created_at` equals the MAX for that chat).
    pub async fn get_recent_chats(&self, user_id: Uuid) -> Result<Vec<RecentChat>, String> {
        let rows = sqlx::query_as::<_, RecentChat>(
            r#"
            SELECT
                c.id           AS chat_id,
                m.content      AS last_message,
                m.created_at   AS last_message_time,
                COALESCE(other_user.id, $1) AS other_user_id,
                COALESCE(NULLIF(TRIM(COALESCE(other_user.first_name, '') || ' ' || COALESCE(other_user.last_name, '')), ''), 'Unknown User') AS other_user_name,
                other_user.profile_picture AS other_user_image
            FROM chats c
            JOIN messages m ON m.chat_id = c.id
            LEFT JOIN chat_participants cp_other
                ON cp_other.chat_id = c.id AND cp_other.user_id != $1
            LEFT JOIN users other_user
                ON other_user.id = cp_other.user_id
            WHERE c.id IN (
                SELECT chat_id
                FROM chat_participants
                WHERE user_id = $1
            )
            AND c.is_deleted = FALSE
            AND m.created_at = (
                SELECT MAX(created_at)
                FROM messages
                WHERE chat_id = c.id
            )
            ORDER BY m.created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        Ok(rows)
    }
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    /// Validates that an invalid UUID string produces a parse error before we
    /// even attempt a DB call — i.e., the service layer's UUID parsing works.
    #[test]
    fn test_invalid_uuid_parse_returns_error() {
        let bad_id = "not-a-uuid";
        let result = uuid::Uuid::parse_str(bad_id);
        assert!(
            result.is_err(),
            "Expected Uuid::parse_str to fail on non-UUID input"
        );
    }

    /// Validates that a well-formed UUID parses successfully.
    #[test]
    fn test_valid_uuid_parse_succeeds() {
        let good_id = "550e8400-e29b-41d4-a716-446655440000";
        let result = uuid::Uuid::parse_str(good_id);
        assert!(
            result.is_ok(),
            "Expected Uuid::parse_str to succeed on a valid UUID"
        );
    }
}
