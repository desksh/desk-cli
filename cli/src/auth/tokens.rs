//! Token types and credential structures for authentication.
//!
//! This module defines the core data structures for OAuth tokens and API credentials:
//! - [`AuthProvider`] - Supported OAuth providers (GitHub, Google)
//! - [`TokenSet`] - OAuth tokens received from a provider
//! - [`ApiCredentials`] - Complete credentials including API token and user info

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Supported OAuth authentication providers.
///
/// Used to identify which OAuth provider was used for authentication
/// and to select the appropriate provider configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AuthProvider {
    /// GitHub OAuth provider.
    #[default]
    GitHub,
    /// Google OAuth provider.
    Google,
}

impl std::fmt::Display for AuthProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitHub => write!(f, "github"),
            Self::Google => write!(f, "google"),
        }
    }
}

impl TryFrom<&str> for AuthProvider {
    type Error = String;

    /// Parses a provider name from a string (case-insensitive).
    ///
    /// # Examples
    ///
    /// ```
    /// use desk::auth::AuthProvider;
    ///
    /// assert_eq!(AuthProvider::try_from("github"), Ok(AuthProvider::GitHub));
    /// assert_eq!(AuthProvider::try_from("Google"), Ok(AuthProvider::Google));
    /// assert!(AuthProvider::try_from("unknown").is_err());
    /// ```
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "github" => Ok(Self::GitHub),
            "google" => Ok(Self::Google),
            _ => Err(format!("Unknown provider: {s}")),
        }
    }
}

/// Token set received from an OAuth provider after successful authentication.
///
/// Contains the access token, optional refresh token, and expiration information.
/// The token type is typically "Bearer" for OAuth 2.0 flows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSet {
    /// The access token for API requests.
    pub access_token: String,
    /// The refresh token for obtaining new access tokens (if provided by the provider).
    pub refresh_token: Option<String>,
    /// Token type, typically "Bearer".
    pub token_type: String,
    /// When the access token expires (if known).
    pub expires_at: Option<DateTime<Utc>>,
    /// Space-separated list of granted OAuth scopes.
    pub scope: Option<String>,
}

impl TokenSet {
    /// Checks if the access token is expired or will expire within 5 minutes.
    ///
    /// Uses a 5-minute buffer to allow for network latency and clock skew.
    /// Returns `false` if no expiration time is set (tokens without expiry).
    #[allow(dead_code)] // Kept for future use in proactive refresh logic
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.expires_at
            .is_some_and(|exp| exp <= Utc::now() + Duration::minutes(5))
    }

    /// Checks if the token can be refreshed using a refresh token.
    #[must_use]
    pub const fn can_refresh(&self) -> bool {
        self.refresh_token.is_some()
    }
}

/// Complete API credentials including provider tokens and desk API token.
///
/// This structure is stored in the OS keyring and contains all information
/// needed to make authenticated API requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCredentials {
    /// The OAuth provider used for authentication.
    pub provider: AuthProvider,
    /// Tokens received from the OAuth provider.
    pub provider_tokens: TokenSet,
    /// Desk API access token (exchanged from provider tokens).
    pub api_token: String,
    /// When the desk API token expires.
    pub api_token_expires_at: DateTime<Utc>,
    /// User ID from the desk backend.
    pub user_id: String,
}

impl ApiCredentials {
    /// Checks if the API token is expired or will expire within 5 minutes.
    ///
    /// Uses a 5-minute buffer to proactively refresh tokens before they expire.
    #[must_use]
    pub fn is_api_token_expired(&self) -> bool {
        self.api_token_expires_at <= Utc::now() + Duration::minutes(5)
    }

    /// Checks if we can refresh the tokens using the provider's refresh token.
    #[must_use]
    pub const fn can_refresh(&self) -> bool {
        self.provider_tokens.can_refresh()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_token_set(
        expires_at: Option<DateTime<Utc>>,
        refresh_token: Option<String>,
    ) -> TokenSet {
        TokenSet {
            access_token: "test_access_token".to_string(),
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_at,
            scope: Some("read:user".to_string()),
        }
    }

    fn make_credentials(
        api_expires_at: DateTime<Utc>,
        refresh_token: Option<String>,
    ) -> ApiCredentials {
        ApiCredentials {
            provider: AuthProvider::GitHub,
            provider_tokens: make_token_set(None, refresh_token),
            api_token: "test_api_token".to_string(),
            api_token_expires_at: api_expires_at,
            user_id: "user_123".to_string(),
        }
    }

    mod auth_provider {
        use super::*;

        #[test]
        fn display_github() {
            assert_eq!(AuthProvider::GitHub.to_string(), "github");
        }

        #[test]
        fn display_google() {
            assert_eq!(AuthProvider::Google.to_string(), "google");
        }

        #[test]
        fn try_from_github() {
            assert_eq!(AuthProvider::try_from("github"), Ok(AuthProvider::GitHub));
            assert_eq!(AuthProvider::try_from("GitHub"), Ok(AuthProvider::GitHub));
            assert_eq!(AuthProvider::try_from("GITHUB"), Ok(AuthProvider::GitHub));
        }

        #[test]
        fn try_from_google() {
            assert_eq!(AuthProvider::try_from("google"), Ok(AuthProvider::Google));
            assert_eq!(AuthProvider::try_from("Google"), Ok(AuthProvider::Google));
        }

        #[test]
        fn try_from_unknown() {
            assert!(AuthProvider::try_from("unknown").is_err());
            assert!(AuthProvider::try_from("gitlab").is_err());
        }

        #[test]
        fn default_is_github() {
            assert_eq!(AuthProvider::default(), AuthProvider::GitHub);
        }

        #[test]
        fn serialization_roundtrip() {
            let provider = AuthProvider::GitHub;
            let json = serde_json::to_string(&provider).expect("serialize");
            assert_eq!(json, "\"github\"");

            let parsed: AuthProvider = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(parsed, provider);
        }
    }

    mod token_set {
        use super::*;

        #[test]
        fn is_expired_when_past() {
            let token = make_token_set(Some(Utc::now() - Duration::hours(1)), None);
            assert!(token.is_expired());
        }

        #[test]
        fn is_expired_within_5_minutes() {
            let token = make_token_set(Some(Utc::now() + Duration::minutes(3)), None);
            assert!(token.is_expired());
        }

        #[test]
        fn is_not_expired_beyond_5_minutes() {
            let token = make_token_set(Some(Utc::now() + Duration::minutes(10)), None);
            assert!(!token.is_expired());
        }

        #[test]
        fn is_not_expired_when_no_expiry() {
            let token = make_token_set(None, None);
            assert!(!token.is_expired());
        }

        #[test]
        fn can_refresh_with_refresh_token() {
            let token = make_token_set(None, Some("refresh_token".to_string()));
            assert!(token.can_refresh());
        }

        #[test]
        fn cannot_refresh_without_refresh_token() {
            let token = make_token_set(None, None);
            assert!(!token.can_refresh());
        }

        #[test]
        fn serialization_roundtrip() {
            let token = make_token_set(
                Some(Utc::now() + Duration::hours(1)),
                Some("refresh".to_string()),
            );
            let json = serde_json::to_string(&token).expect("serialize");
            let parsed: TokenSet = serde_json::from_str(&json).expect("deserialize");

            assert_eq!(parsed.access_token, token.access_token);
            assert_eq!(parsed.refresh_token, token.refresh_token);
            assert_eq!(parsed.token_type, token.token_type);
        }
    }

    mod api_credentials {
        use super::*;

        #[test]
        fn is_api_token_expired_when_past() {
            let creds = make_credentials(Utc::now() - Duration::hours(1), None);
            assert!(creds.is_api_token_expired());
        }

        #[test]
        fn is_api_token_expired_within_5_minutes() {
            let creds = make_credentials(Utc::now() + Duration::minutes(3), None);
            assert!(creds.is_api_token_expired());
        }

        #[test]
        fn is_api_token_not_expired_beyond_5_minutes() {
            let creds = make_credentials(Utc::now() + Duration::minutes(10), None);
            assert!(!creds.is_api_token_expired());
        }

        #[test]
        fn can_refresh_delegates_to_token_set() {
            let with_refresh = make_credentials(Utc::now(), Some("refresh".to_string()));
            let without_refresh = make_credentials(Utc::now(), None);

            assert!(with_refresh.can_refresh());
            assert!(!without_refresh.can_refresh());
        }

        #[test]
        fn serialization_roundtrip() {
            let creds =
                make_credentials(Utc::now() + Duration::hours(1), Some("refresh".to_string()));
            let json = serde_json::to_string(&creds).expect("serialize");
            let parsed: ApiCredentials = serde_json::from_str(&json).expect("deserialize");

            assert_eq!(parsed.provider, creds.provider);
            assert_eq!(parsed.api_token, creds.api_token);
            assert_eq!(parsed.user_id, creds.user_id);
        }
    }
}
