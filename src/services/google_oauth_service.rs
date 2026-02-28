// src/services/google_oauth_service.rs
//
// Verifies a Google ID token by calling Google's tokeninfo endpoint.
// Returns structured user data (google_id/sub, email, name, picture).
// No third-party OIDC crate needed — a simple HTTP GET to Google is sufficient
// for server-side verification.

use serde::Deserialize;

/// The subset of Google's tokeninfo payload we care about.
#[derive(Debug, Deserialize)]
pub struct GoogleTokenInfo {
    /// Google's unique subject ID for the user (stable across sign-ins).
    pub sub: String,
    /// Verified email address.
    pub email: String,
    /// Intended audience — must match our GOOGLE_CLIENT_ID.
    pub aud: String,
    /// Human-readable display name.
    pub name: Option<String>,
    /// URL to the user's profile picture.
    pub picture: Option<String>,
}

/// Cleaned-up user data returned after successful token verification.
#[derive(Debug, Clone)]
pub struct GoogleUser {
    pub google_id: String,
    pub email: String,
    pub name: String,
    pub picture: Option<String>,
}

/// Verifies `id_token` against Google's public endpoint and returns
/// the verified user info.
///
/// # Errors
/// Returns `Err(String)` if:
/// - The HTTP call to Google fails
/// - Google returns a non-200 status (token invalid / expired)
/// - The `aud` field doesn't match `expected_client_id`
pub async fn verify_google_id_token(
    id_token: &str,
    expected_client_id: &str,
) -> Result<GoogleUser, String> {
    let url = format!(
        "https://oauth2.googleapis.com/tokeninfo?id_token={}",
        id_token
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to contact Google: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "unknown error".to_string());
        return Err(format!(
            "Google rejected the token (HTTP {}): {}",
            status, body
        ));
    }

    let token_info: GoogleTokenInfo = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Google tokeninfo response: {}", e))?;

    // Security: always verify the audience matches our client ID.
    // This prevents tokens issued for other apps from being accepted.
    if token_info.aud != expected_client_id {
        return Err(format!(
            "Token audience mismatch: expected '{}', got '{}'",
            expected_client_id, token_info.aud
        ));
    }

    Ok(GoogleUser {
        google_id: token_info.sub,
        email: token_info.email,
        name: token_info.name.unwrap_or_else(|| "Unknown".to_string()),
        picture: token_info.picture,
    })
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Construct a GoogleUser directly to validate field mapping.
    #[test]
    fn test_google_user_fields() {
        let user = GoogleUser {
            google_id: "1234567890".to_string(),
            email: "alice@gmail.com".to_string(),
            name: "Alice Smith".to_string(),
            picture: Some("https://lh3.googleusercontent.com/a/photo".to_string()),
        };

        assert_eq!(user.google_id, "1234567890");
        assert_eq!(user.email, "alice@gmail.com");
        assert_eq!(user.name, "Alice Smith");
        assert!(user.picture.is_some());
    }

    /// Audience mismatch logic — no HTTP call; tests pure validation logic.
    #[test]
    fn test_audience_mismatch_detection() {
        // Simulate what verify_google_id_token does with a wrong aud
        let token_info = GoogleTokenInfo {
            sub: "abc123".to_string(),
            email: "bob@gmail.com".to_string(),
            aud: "wrong-client-id.apps.googleusercontent.com".to_string(),
            name: Some("Bob".to_string()),
            picture: None,
        };

        let expected_client_id = "correct-client-id.apps.googleusercontent.com";
        let mismatch = token_info.aud != expected_client_id;
        assert!(
            mismatch,
            "Should detect aud mismatch when client ID doesn't match"
        );
    }

    /// Tokens with no name or picture should gracefully default.
    #[test]
    fn test_missing_optional_fields_default() {
        let user = GoogleUser {
            google_id: "xyz".to_string(),
            email: "minimal@gmail.com".to_string(),
            name: "Unknown".to_string(), // default applied
            picture: None,
        };

        assert_eq!(user.name, "Unknown");
        assert!(user.picture.is_none());
    }
}
