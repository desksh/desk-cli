//! Token refresh middleware for automatic token renewal.
//!
//! This middleware intercepts HTTP requests and:
//! - Proactively refreshes tokens that are about to expire
//! - Logs warnings when 401 responses indicate session expiration
//!
//! Token refresh is best-effort; failures are logged but don't block requests.

use std::sync::Arc;

use async_trait::async_trait;
use http::Extensions;
use reqwest::{Client, Request, Response, StatusCode};
use reqwest_middleware::{Middleware, Next, Result};
use tokio::sync::RwLock;
use url::Url;

use crate::auth::{ApiCredentials, CredentialStore};
use crate::error::DeskError;

/// Response structure from the token refresh endpoint.
#[derive(serde::Deserialize)]
struct RefreshResponse {
    api_token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

/// Middleware that automatically refreshes expired API tokens.
///
/// Checks token expiration before each request and attempts to refresh
/// proactively. Uses a 5-minute buffer to ensure tokens don't expire
/// during request processing.
pub struct TokenRefreshMiddleware {
    credentials: Arc<RwLock<Option<ApiCredentials>>>,
    base_url: Url,
    inner_client: Client,
}

impl TokenRefreshMiddleware {
    /// Creates a new token refresh middleware.
    ///
    /// # Arguments
    ///
    /// * `credentials` - Shared credentials storage
    /// * `base_url` - Base URL for the refresh endpoint
    /// * `inner_client` - HTTP client for refresh requests (without middleware)
    #[must_use]
    pub const fn new(
        credentials: Arc<RwLock<Option<ApiCredentials>>>,
        base_url: Url,
        inner_client: Client,
    ) -> Self {
        Self {
            credentials,
            base_url,
            inner_client,
        }
    }

    /// Attempts to refresh the API token using the stored refresh token.
    ///
    /// On success, updates both in-memory credentials and persistent storage.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No credentials are loaded ([`DeskError::NotAuthenticated`])
    /// - No refresh token is available ([`DeskError::TokenExpired`])
    /// - The refresh request fails ([`DeskError::TokenRefreshFailed`])
    /// - The API returns 401 ([`DeskError::Unauthorized`])
    #[allow(clippy::significant_drop_tightening)] // Guards held for related operations
    async fn refresh_token(&self) -> std::result::Result<(), DeskError> {
        // Extract data needed for refresh request, then drop the guard
        let (api_token, refresh_token) = {
            let guard = self.credentials.read().await;
            let creds = guard.as_ref().ok_or(DeskError::NotAuthenticated)?;

            if !creds.can_refresh() {
                return Err(DeskError::TokenExpired);
            }

            (
                creds.api_token.clone(),
                creds.provider_tokens.refresh_token.clone(),
            )
        };

        let refresh_url = self
            .base_url
            .join("/v1/auth/refresh")
            .map_err(|e| DeskError::Config(format!("Invalid refresh URL: {e}")))?;

        let response = self
            .inner_client
            .post(refresh_url)
            .bearer_auth(&api_token)
            .json(&serde_json::json!({
                "refresh_token": refresh_token
            }))
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                401 => DeskError::Unauthorized,
                503 => DeskError::ApiUnavailable,
                _ => DeskError::TokenRefreshFailed(format!("Server returned {status}")),
            });
        }

        let refresh_data: RefreshResponse = response
            .json()
            .await
            .map_err(|e| DeskError::TokenRefreshFailed(format!("Invalid response: {e}")))?;

        // Re-acquire write lock to update credentials
        let mut creds_guard = self.credentials.write().await;
        if let Some(creds) = creds_guard.as_mut() {
            creds.api_token = refresh_data.api_token;
            creds.api_token_expires_at = refresh_data.expires_at;
            CredentialStore::new()?.save(creds)?;
        }

        tracing::debug!("Successfully refreshed API token");
        Ok(())
    }

    /// Checks if credentials need refresh and attempts it proactively.
    ///
    /// This is a best-effort operation that logs failures but doesn't
    /// propagate errors to the caller.
    async fn try_proactive_refresh(&self) {
        let needs_refresh = {
            let creds = self.credentials.read().await;
            creds
                .as_ref()
                .is_some_and(|c| c.is_api_token_expired() && c.can_refresh())
        };

        if needs_refresh {
            if let Err(e) = self.refresh_token().await {
                tracing::debug!("Proactive token refresh failed: {e}");
            }
        }
    }
}

#[async_trait]
impl Middleware for TokenRefreshMiddleware {
    /// Handles an HTTP request, refreshing tokens if needed.
    ///
    /// Checks for token expiration before the request and logs warnings
    /// if the response indicates authentication failure.
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> Result<Response> {
        self.try_proactive_refresh().await;

        let response = next.run(req, extensions).await?;

        if response.status() == StatusCode::UNAUTHORIZED {
            tracing::warn!("Received 401 Unauthorized. Run 'desk auth login' to re-authenticate.");
        }

        Ok(response)
    }
}
