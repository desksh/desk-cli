//! Application configuration settings.

use serde::{Deserialize, Serialize};
use url::Url;

use crate::auth::tokens::AuthProvider;

/// Main configuration for desk-cli.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DeskConfig {
    /// Authentication settings.
    pub auth: AuthConfig,
    /// API client settings.
    pub api: ApiConfig,
}

/// Authentication configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    /// Default OAuth provider.
    pub default_provider: AuthProvider,
    /// Provider-specific settings.
    pub providers: ProvidersConfig,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            default_provider: AuthProvider::GitHub,
            providers: ProvidersConfig::default(),
        }
    }
}

/// OAuth provider configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProvidersConfig {
    /// GitHub OAuth settings.
    pub github: ProviderConfig,
    /// Google OAuth settings.
    pub google: ProviderConfig,
}

impl Default for ProvidersConfig {
    fn default() -> Self {
        Self {
            github: ProviderConfig {
                enabled: true,
                client_id: None,
            },
            google: ProviderConfig {
                enabled: true,
                client_id: None,
            },
        }
    }
}

/// Individual OAuth provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Whether this provider is enabled.
    pub enabled: bool,
    /// Optional custom client ID (for enterprise setups).
    pub client_id: Option<String>,
}

/// API client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ApiConfig {
    /// Backend API base URL.
    #[serde(with = "url_serde")]
    pub base_url: Url,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
    /// Maximum number of retries for failed requests.
    pub max_retries: u32,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            base_url: Url::parse("https://api.getdesk.dev").expect("valid default URL"),
            timeout_secs: 30,
            max_retries: 3,
        }
    }
}

/// Custom serde module for URL serialization.
mod url_serde {
    use serde::{self, Deserialize, Deserializer, Serializer};
    use url::Url;

    pub fn serialize<S>(url: &Url, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(url.as_str())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Url, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Url::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Environment variables that can override configuration.
#[allow(dead_code)]
pub mod env {
    pub const API_URL: &str = "DESK_API_URL";
    pub const AUTH_PROVIDER: &str = "DESK_AUTH_PROVIDER";
    pub const LOG_LEVEL: &str = "DESK_LOG";
}

impl DeskConfig {
    /// Apply environment variable overrides to the configuration.
    #[must_use]
    pub fn with_env_overrides(mut self) -> Self {
        if let Ok(url) = std::env::var(env::API_URL) {
            if let Ok(parsed) = Url::parse(&url) {
                self.api.base_url = parsed;
            }
        }

        if let Ok(provider) = std::env::var(env::AUTH_PROVIDER) {
            if let Ok(p) = provider.to_lowercase().as_str().try_into() {
                self.auth.default_provider = p;
            }
        }

        self
    }
}
