//! Error types and result aliases for desk-cli.
//!
//! This module provides a comprehensive error handling system with:
//! - Specific error variants for different failure modes
//! - User-friendly error messages with recovery suggestions
//! - Helper methods for error classification
//! - Automatic conversion from common error types

use thiserror::Error;

use crate::git::GitError;
use crate::workspace::WorkspaceError;

/// Main error type for desk-cli operations.
///
/// Each variant includes a user-friendly message with actionable recovery steps.
/// Use [`requires_reauth`](Self::requires_reauth) and [`is_retriable`](Self::is_retriable)
/// to determine appropriate error handling strategies.
#[derive(Error, Debug)]
pub enum DeskError {
    /// User is not authenticated. Credentials not found in keyring.
    #[error("Not authenticated. Run 'desk auth login' to authenticate.")]
    NotAuthenticated,

    /// API token has expired and could not be refreshed.
    #[error("Your session has expired. Run 'desk auth login' to re-authenticate.")]
    TokenExpired,

    /// Stored credentials are malformed or corrupted.
    #[error("Invalid credentials. Your stored credentials may be corrupted. Try 'desk auth logout' then 'desk auth login'.")]
    InvalidCredentials,

    /// OAuth or API authentication failed.
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Token refresh request failed.
    #[error("Token refresh failed. Run 'desk auth login' to re-authenticate. Details: {0}")]
    TokenRefreshFailed(String),

    /// Device authorization code expired before user completed authentication.
    #[error("Device authorization code expired. Please run 'desk auth login' again and complete authorization within the time limit.")]
    DeviceAuthorizationExpired,

    /// User denied the authorization request.
    #[error(
        "Authorization was denied. If this was unintentional, run 'desk auth login' to try again."
    )]
    AccessDenied,

    /// OAuth provider is unreachable.
    #[error("OAuth provider '{provider}' is unavailable. Check your network connection or try again later.")]
    ProviderUnavailable {
        /// The name of the unavailable provider.
        provider: String,
    },

    /// API returned a non-success status code.
    #[error("API request failed ({status}): {message}")]
    ApiError {
        /// HTTP status code.
        status: u16,
        /// Error message from the API.
        message: String,
    },

    /// API returned 401 Unauthorized.
    #[error("API server returned unauthorized (401). Your session may have expired. Run 'desk auth login' to re-authenticate.")]
    Unauthorized,

    /// API server is unreachable (503 or connection failed).
    #[error("API server is unavailable. Check your network connection or try again later.")]
    ApiUnavailable,

    /// Request timed out.
    #[error("Request timed out. The server may be slow or unreachable. Try again later.")]
    Timeout,

    /// Network error during HTTP request.
    #[error("Network error: {0}. Check your internet connection.")]
    Network(String),

    /// Failed to access the OS keyring.
    #[error("Failed to access credential storage: {0}. Ensure your system keyring is unlocked.")]
    CredentialStorage(String),

    /// General configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Failed to read configuration file.
    #[error("Failed to read configuration file: {0}. Check file permissions and format.")]
    ConfigRead(String),

    /// Failed to write configuration file.
    #[error("Failed to write configuration file: {0}. Check directory permissions.")]
    ConfigWrite(String),

    /// IO operation failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON or TOML serialization/deserialization failed.
    #[error("Data serialization error: {0}. This may indicate corrupted data.")]
    Serialization(String),

    /// URL parsing failed.
    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    /// Workspace operation error.
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),

    /// Git operation error.
    #[error(transparent)]
    Git(#[from] GitError),
}

impl DeskError {
    /// Checks if this error can be resolved by re-authenticating.
    ///
    /// Returns `true` for errors related to missing, expired, or invalid credentials.
    /// Use this to determine when to prompt the user to run `desk auth login`.
    #[allow(dead_code)] // Kept for future use in main error handler
    #[must_use]
    pub const fn requires_reauth(&self) -> bool {
        matches!(
            self,
            Self::NotAuthenticated
                | Self::TokenExpired
                | Self::InvalidCredentials
                | Self::Unauthorized
                | Self::AccessDenied
        )
    }

    /// Checks if this error is transient and the operation might succeed on retry.
    ///
    /// Returns `true` for network-related errors and service unavailability.
    /// Use this to implement retry logic with exponential backoff.
    #[allow(dead_code)] // Kept for future use in API client retry logic
    #[must_use]
    pub const fn is_retriable(&self) -> bool {
        matches!(
            self,
            Self::Network(_)
                | Self::Timeout
                | Self::ApiUnavailable
                | Self::ProviderUnavailable { .. }
        )
    }
}

/// Result type alias using [`DeskError`].
pub type Result<T> = std::result::Result<T, DeskError>;

impl From<serde_json::Error> for DeskError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(format!("JSON error: {err}"))
    }
}

impl From<toml::de::Error> for DeskError {
    fn from(err: toml::de::Error) -> Self {
        Self::ConfigRead(format!("TOML parse error: {err}"))
    }
}

impl From<toml::ser::Error> for DeskError {
    fn from(err: toml::ser::Error) -> Self {
        Self::ConfigWrite(format!("TOML serialize error: {err}"))
    }
}

impl From<keyring::Error> for DeskError {
    fn from(err: keyring::Error) -> Self {
        Self::CredentialStorage(err.to_string())
    }
}

impl From<reqwest::Error> for DeskError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::Timeout
        } else if err.is_connect() {
            Self::ApiUnavailable
        } else {
            Self::Network(err.to_string())
        }
    }
}

impl From<reqwest_middleware::Error> for DeskError {
    fn from(err: reqwest_middleware::Error) -> Self {
        let err_str = err.to_string();
        if err_str.contains("timeout") || err_str.contains("timed out") {
            Self::Timeout
        } else if err_str.contains("connect") || err_str.contains("connection") {
            Self::ApiUnavailable
        } else {
            Self::Network(err_str)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_are_user_friendly() {
        let not_auth = DeskError::NotAuthenticated;
        assert!(not_auth.to_string().contains("desk auth login"));

        let expired = DeskError::TokenExpired;
        assert!(expired.to_string().contains("desk auth login"));

        let device_expired = DeskError::DeviceAuthorizationExpired;
        assert!(device_expired.to_string().contains("desk auth login"));

        let unauthorized = DeskError::Unauthorized;
        assert!(unauthorized.to_string().contains("desk auth login"));
    }

    #[test]
    fn api_error_includes_status_and_message() {
        let err = DeskError::ApiError {
            status: 404,
            message: "Not found".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("404"));
        assert!(msg.contains("Not found"));
    }

    #[test]
    fn requires_reauth_identifies_auth_errors() {
        assert!(DeskError::NotAuthenticated.requires_reauth());
        assert!(DeskError::TokenExpired.requires_reauth());
        assert!(DeskError::Unauthorized.requires_reauth());
        assert!(DeskError::InvalidCredentials.requires_reauth());
        assert!(DeskError::AccessDenied.requires_reauth());

        assert!(!DeskError::Timeout.requires_reauth());
        assert!(!DeskError::ApiUnavailable.requires_reauth());
        assert!(!DeskError::Network("test".to_string()).requires_reauth());
    }

    #[test]
    fn is_retriable_identifies_transient_errors() {
        assert!(DeskError::Timeout.is_retriable());
        assert!(DeskError::ApiUnavailable.is_retriable());
        assert!(DeskError::Network("test".to_string()).is_retriable());
        assert!(DeskError::ProviderUnavailable {
            provider: "github".to_string()
        }
        .is_retriable());

        assert!(!DeskError::NotAuthenticated.is_retriable());
        assert!(!DeskError::TokenExpired.is_retriable());
        assert!(!DeskError::InvalidCredentials.is_retriable());
    }

    #[test]
    fn from_serde_json_error() {
        let json_err = serde_json::from_str::<String>("invalid").unwrap_err();
        let desk_err: DeskError = json_err.into();
        assert!(matches!(desk_err, DeskError::Serialization(_)));
    }

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let desk_err: DeskError = io_err.into();
        assert!(matches!(desk_err, DeskError::Io(_)));
    }

    #[test]
    fn from_url_parse_error() {
        let url_err = url::Url::parse("not a url").unwrap_err();
        let desk_err: DeskError = url_err.into();
        assert!(matches!(desk_err, DeskError::InvalidUrl(_)));
    }
}
