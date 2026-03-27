// src/services/chat_db_service.rs
//
// Service layer wrapping ChatRepository for the recent-chats feature.
// Converts the string user_id (from the JWT extractor) to a UUID,
// then delegates to the repository.

use crate::models::recent_chat::RecentChat;
use crate::repository::chat_repository::ChatRepository;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ChatDbService {
    pub repo: ChatRepository,
}

impl ChatDbService {
    pub fn new(repo: ChatRepository) -> Self {
        Self { repo }
    }

    /// Fetch recent chats for `user_id` (UUID as string from JWT claims).
    /// Returns an Err if the user_id string isn't a valid UUID.
    pub async fn get_recent_chats(&self, user_id: &str) -> Result<Vec<RecentChat>, String> {
        let uuid = Uuid::parse_str(user_id)
            .map_err(|e| format!("Invalid user ID '{}': {}", user_id, e))?;

        self.repo.get_recent_chats(uuid).await
    }
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// uuid::Uuid::parse_str is the core guard — test both branches
    /// without needing a live DB connection.

    #[test]
    fn test_invalid_user_id_produces_error() {
        let bad_id = "definitely-not-a-uuid";
        let result = Uuid::parse_str(bad_id);
        assert!(
            result.is_err(),
            "Parsing a non-UUID string should fail, got: {:?}",
            result
        );
    }

    #[test]
    fn test_valid_user_id_parses_correctly() {
        let good_id = "550e8400-e29b-41d4-a716-446655440000";
        let result = Uuid::parse_str(good_id);
        assert!(result.is_ok(), "Parsing a valid UUID string should succeed");
        assert_eq!(result.unwrap().to_string(), good_id);
    }

    #[test]
    fn test_nil_uuid_is_valid() {
        let nil_id = "00000000-0000-0000-0000-000000000000";
        let result = Uuid::parse_str(nil_id);
        assert!(result.is_ok());
    }
}
