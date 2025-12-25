//! Desk API client for backend communication.
//!
//! This module provides the main HTTP client for interacting with the Desk API.
//! It handles:
//! - Authentication header injection via middleware
//! - Automatic token refresh for expired tokens
//! - OAuth token exchange for API credentials

use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use tokio::sync::RwLock;
use url::Url;

use crate::auth::{ApiCredentials, AuthProvider, CredentialStore, TokenSet};
use crate::client::middleware::{AuthMiddleware, TokenRefreshMiddleware};
use crate::config::ApiConfig;
use crate::error::{DeskError, Result};

/// Response from the token exchange endpoint.
#[derive(serde::Deserialize)]
struct TokenExchangeResponse {
    api_token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
    user_id: String,
}

/// Main API client for communicating with the Desk backend.
///
/// Wraps an HTTP client with authentication and token refresh middleware.
/// Credentials are stored in memory and synchronized with the OS keyring.
pub struct DeskApiClient {
    client: ClientWithMiddleware,
    base_url: Url,
    credentials: Arc<RwLock<Option<ApiCredentials>>>,
}

impl DeskApiClient {
    /// Creates a new API client with the given configuration.
    ///
    /// Initializes the HTTP client with authentication and token refresh middleware.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built (e.g., TLS initialization fails).
    pub fn new(config: &ApiConfig) -> Result<Self> {
        let inner_client = Client::builder()
            .user_agent(format!("desk-cli/{}", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()?;

        let credentials: Arc<RwLock<Option<ApiCredentials>>> = Arc::new(RwLock::new(None));

        let client = ClientBuilder::new(inner_client.clone())
            .with(AuthMiddleware::new(Arc::clone(&credentials)))
            .with(TokenRefreshMiddleware::new(
                Arc::clone(&credentials),
                config.base_url.clone(),
                inner_client,
            ))
            .build();

        Ok(Self {
            client,
            base_url: config.base_url.clone(),
            credentials,
        })
    }

    /// Loads credentials from the OS keyring into memory.
    ///
    /// Should be called before making authenticated API requests.
    ///
    /// # Returns
    ///
    /// Returns `true` if credentials were found and loaded, `false` if none exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the keyring is inaccessible or credentials are corrupted.
    pub async fn load_credentials(&self) -> Result<bool> {
        let store = CredentialStore::new()?;
        if let Some(creds) = store.load()? {
            *self.credentials.write().await = Some(creds);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Sets credentials directly in memory (after successful authentication).
    ///
    /// Does not persist to the keyring; use [`CredentialStore::save`] for that.
    #[allow(dead_code)] // Kept for future API commands
    pub async fn set_credentials(&self, creds: ApiCredentials) {
        *self.credentials.write().await = Some(creds);
    }

    /// Clears credentials from memory.
    ///
    /// Does not delete from the keyring; use [`CredentialStore::delete`] for that.
    #[allow(dead_code)] // Kept for future logout enhancements
    pub async fn clear_credentials(&self) {
        *self.credentials.write().await = None;
    }

    /// Checks if the client has credentials loaded in memory.
    #[allow(dead_code)] // Kept for future API commands
    pub async fn is_authenticated(&self) -> bool {
        self.credentials.read().await.is_some()
    }

    /// Returns a clone of the current credentials, if any.
    pub async fn get_credentials(&self) -> Option<ApiCredentials> {
        self.credentials.read().await.clone()
    }

    /// Exchanges OAuth provider tokens for Desk API credentials.
    ///
    /// Called after successful OAuth authentication to obtain API access.
    ///
    /// # Arguments
    ///
    /// * `provider` - The OAuth provider used for authentication
    /// * `provider_tokens` - Tokens received from the OAuth provider
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The API is unreachable ([`DeskError::ApiUnavailable`])
    /// - Authentication fails ([`DeskError::Unauthorized`])
    /// - The response cannot be parsed ([`DeskError::Serialization`])
    pub async fn exchange_token(
        &self,
        provider: AuthProvider,
        provider_tokens: &TokenSet,
    ) -> Result<ApiCredentials> {
        let url = self
            .base_url
            .join("/v1/auth/token")
            .map_err(|e| DeskError::Config(format!("Invalid token URL: {e}")))?;

        let body = serde_json::json!({
            "provider": provider.to_string(),
            "access_token": provider_tokens.access_token,
            "refresh_token": provider_tokens.refresh_token,
        });

        let response = self
            .client
            .post(url)
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_string(&body)?)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let status_code = status.as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| format!("HTTP {status_code}"));

            return Err(match status_code {
                401 => DeskError::Unauthorized,
                503 => DeskError::ApiUnavailable,
                _ => DeskError::ApiError {
                    status: status_code,
                    message,
                },
            });
        }

        let data: TokenExchangeResponse = response
            .json()
            .await
            .map_err(|e| DeskError::Serialization(format!("Invalid token response: {e}")))?;

        Ok(ApiCredentials {
            provider,
            provider_tokens: provider_tokens.clone(),
            api_token: data.api_token,
            api_token_expires_at: data.expires_at,
            user_id: data.user_id,
        })
    }

    /// Returns the base URL for API requests.
    #[allow(dead_code)] // Kept for future API commands
    #[must_use]
    pub const fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Returns a reference to the underlying HTTP client.
    ///
    /// Use this for making custom API requests not covered by other methods.
    #[allow(dead_code)] // Kept for future API commands
    #[must_use]
    pub const fn client(&self) -> &ClientWithMiddleware {
        &self.client
    }
}
