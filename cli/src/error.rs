//! Application error types for desk-cli.

use thiserror::Error;

/// Main error type for desk-cli operations.
#[derive(Error, Debug)]
pub enum DeskError {
    // Authentication errors
    #[error("Not authenticated. Run 'desk auth login' first.")]
    NotAuthenticated,

    #[allow(dead_code)]
    #[error("Authentication expired. Please login again.")]
    AuthenticationExpired,

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Token refresh failed: {0}")]
    TokenRefreshFailed(String),

    #[error("Device authorization expired. Please try again.")]
    DeviceAuthorizationExpired,

    #[error("Access denied by user.")]
    AccessDenied,

    // API errors
    #[error("API request failed: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Network error: {0}")]
    Network(String),

    // Credential storage errors
    #[error("Failed to access credential storage: {0}")]
    CredentialStorage(String),

    // Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Failed to read configuration file: {0}")]
    ConfigRead(String),

    #[error("Failed to write configuration file: {0}")]
    ConfigWrite(String),

    // IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    // Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(String),

    // URL parsing errors
    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
}

/// Result type alias using `DeskError`.
pub type Result<T> = std::result::Result<T, DeskError>;

impl From<serde_json::Error> for DeskError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

impl From<toml::de::Error> for DeskError {
    fn from(err: toml::de::Error) -> Self {
        Self::ConfigRead(err.to_string())
    }
}

impl From<toml::ser::Error> for DeskError {
    fn from(err: toml::ser::Error) -> Self {
        Self::ConfigWrite(err.to_string())
    }
}

impl From<keyring::Error> for DeskError {
    fn from(err: keyring::Error) -> Self {
        Self::CredentialStorage(err.to_string())
    }
}

impl From<reqwest::Error> for DeskError {
    fn from(err: reqwest::Error) -> Self {
        Self::Network(err.to_string())
    }
}

impl From<reqwest_middleware::Error> for DeskError {
    fn from(err: reqwest_middleware::Error) -> Self {
        Self::Network(err.to_string())
    }
}
