//! Desk API client implementation.

use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use tokio::sync::RwLock;
use url::Url;

use crate::auth::{ApiCredentials, CredentialStore, TokenSet};
use crate::client::middleware::{AuthMiddleware, TokenRefreshMiddleware};
use crate::config::ApiConfig;
use crate::error::{DeskError, Result};

/// Main API client for communicating with the Desk backend.
pub struct DeskApiClient {
    client: ClientWithMiddleware,
    base_url: Url,
    credentials: Arc<RwLock<Option<ApiCredentials>>>,
}

impl DeskApiClient {
    /// Create a new API client.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built.
    pub fn new(config: &ApiConfig) -> Result<Self> {
        let inner_client = Client::builder()
            .user_agent(format!("desk-cli/{}", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()?;

        let credentials: Arc<RwLock<Option<ApiCredentials>>> = Arc::new(RwLock::new(None));

        // Build middleware stack
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

    /// Load credentials from secure storage.
    ///
    /// # Returns
    ///
    /// Returns `true` if credentials were found and loaded.
    ///
    /// # Errors
    ///
    /// Returns an error if credentials cannot be read.
    pub async fn load_credentials(&self) -> Result<bool> {
        let store = CredentialStore::new()?;
        if let Some(creds) = store.load()? {
            *self.credentials.write().await = Some(creds);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Set credentials directly (after authentication).
    #[allow(dead_code)]
    pub async fn set_credentials(&self, creds: ApiCredentials) {
        *self.credentials.write().await = Some(creds);
    }

    /// Clear credentials.
    #[allow(dead_code)]
    pub async fn clear_credentials(&self) {
        *self.credentials.write().await = None;
    }

    /// Check if the client has valid credentials loaded.
    #[allow(dead_code)]
    pub async fn is_authenticated(&self) -> bool {
        self.credentials.read().await.is_some()
    }

    /// Get the current user information.
    pub async fn get_credentials(&self) -> Option<ApiCredentials> {
        self.credentials.read().await.clone()
    }

    /// Exchange OAuth provider tokens for Desk API tokens.
    ///
    /// This is called after successful OAuth authentication to get
    /// Desk backend API credentials.
    ///
    /// # Errors
    ///
    /// Returns an error if the token exchange fails.
    pub async fn exchange_token(
        &self,
        provider: crate::auth::AuthProvider,
        provider_tokens: &TokenSet,
    ) -> Result<ApiCredentials> {
        let url = self
            .base_url
            .join("/v1/auth/token")
            .map_err(|e| DeskError::Config(format!("Invalid URL: {e}")))?;

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

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(DeskError::ApiError { status, message });
        }

        #[derive(serde::Deserialize)]
        struct TokenExchangeResponse {
            api_token: String,
            expires_at: chrono::DateTime<chrono::Utc>,
            user_id: String,
        }

        let data: TokenExchangeResponse = response
            .json()
            .await
            .map_err(|e| DeskError::Serialization(e.to_string()))?;

        Ok(ApiCredentials {
            provider,
            provider_tokens: provider_tokens.clone(),
            api_token: data.api_token,
            api_token_expires_at: data.expires_at,
            user_id: data.user_id,
        })
    }

    /// Get the base URL.
    #[must_use]
    #[allow(dead_code)]
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Get a reference to the underlying HTTP client.
    #[must_use]
    #[allow(dead_code)]
    pub fn client(&self) -> &ClientWithMiddleware {
        &self.client
    }
}
