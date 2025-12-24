//! Token types for authentication.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Supported OAuth authentication providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AuthProvider {
    #[default]
    GitHub,
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

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "github" => Ok(Self::GitHub),
            "google" => Ok(Self::Google),
            _ => Err(format!("Unknown provider: {s}")),
        }
    }
}

/// Token set received from an OAuth provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSet {
    /// The access token.
    pub access_token: String,
    /// The refresh token (if provided).
    pub refresh_token: Option<String>,
    /// Token type (usually "Bearer").
    pub token_type: String,
    /// When the access token expires.
    pub expires_at: Option<DateTime<Utc>>,
    /// Granted scopes.
    pub scope: Option<String>,
}

impl TokenSet {
    /// Check if the access token is expired or will expire within 5 minutes.
    #[must_use]
    #[allow(dead_code)]
    pub fn is_expired(&self) -> bool {
        self.expires_at
            .map(|exp| exp <= Utc::now() + Duration::minutes(5))
            .unwrap_or(false)
    }

    /// Check if the token can be refreshed.
    #[must_use]
    pub fn can_refresh(&self) -> bool {
        self.refresh_token.is_some()
    }
}

/// Complete API credentials including provider tokens and desk API token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCredentials {
    /// The OAuth provider used for authentication.
    pub provider: AuthProvider,
    /// Tokens from the OAuth provider.
    pub provider_tokens: TokenSet,
    /// Desk API access token.
    pub api_token: String,
    /// When the API token expires.
    pub api_token_expires_at: DateTime<Utc>,
    /// User ID from the desk backend.
    pub user_id: String,
}

impl ApiCredentials {
    /// Check if the API token is expired or will expire within 5 minutes.
    #[must_use]
    pub fn is_api_token_expired(&self) -> bool {
        self.api_token_expires_at <= Utc::now() + Duration::minutes(5)
    }

    /// Check if we can refresh the tokens.
    #[must_use]
    pub fn can_refresh(&self) -> bool {
        self.provider_tokens.can_refresh()
    }
}
