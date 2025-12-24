//! Token refresh middleware for automatic token renewal.

use std::sync::Arc;

use async_trait::async_trait;
use http::Extensions;
use reqwest::{Client, Request, Response, StatusCode};
use reqwest_middleware::{Middleware, Next, Result};
use tokio::sync::RwLock;
use url::Url;

use crate::auth::{ApiCredentials, CredentialStore};
use crate::error::DeskError;

/// Middleware that automatically refreshes expired tokens.
pub struct TokenRefreshMiddleware {
    credentials: Arc<RwLock<Option<ApiCredentials>>>,
    base_url: Url,
    inner_client: Client,
}

impl TokenRefreshMiddleware {
    /// Create a new token refresh middleware.
    #[must_use]
    pub fn new(
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

    /// Attempt to refresh the API token.
    async fn refresh_token(&self) -> std::result::Result<(), DeskError> {
        let mut creds_guard = self.credentials.write().await;

        let creds = creds_guard
            .as_mut()
            .ok_or(DeskError::NotAuthenticated)?;

        // Check if we can refresh
        if !creds.can_refresh() {
            return Err(DeskError::TokenRefreshFailed(
                "No refresh token available".to_string(),
            ));
        }

        // Call the backend refresh endpoint
        let refresh_url = self
            .base_url
            .join("/v1/auth/refresh")
            .map_err(|e| DeskError::TokenRefreshFailed(e.to_string()))?;

        let response = self
            .inner_client
            .post(refresh_url)
            .bearer_auth(&creds.api_token)
            .json(&serde_json::json!({
                "refresh_token": creds.provider_tokens.refresh_token
            }))
            .send()
            .await
            .map_err(|e| DeskError::TokenRefreshFailed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DeskError::TokenRefreshFailed(format!(
                "Refresh request failed: {}",
                response.status()
            )));
        }

        #[derive(serde::Deserialize)]
        struct RefreshResponse {
            api_token: String,
            expires_at: chrono::DateTime<chrono::Utc>,
        }

        let refresh_data: RefreshResponse = response
            .json()
            .await
            .map_err(|e| DeskError::TokenRefreshFailed(e.to_string()))?;

        // Update credentials
        creds.api_token = refresh_data.api_token;
        creds.api_token_expires_at = refresh_data.expires_at;

        // Persist updated credentials
        let store = CredentialStore::new()?;
        store.save(creds)?;

        Ok(())
    }
}

#[async_trait]
impl Middleware for TokenRefreshMiddleware {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> Result<Response> {
        // Check if token needs proactive refresh before making the request
        {
            let creds = self.credentials.read().await;
            if let Some(c) = creds.as_ref() {
                if c.is_api_token_expired() && c.can_refresh() {
                    drop(creds);
                    // Best effort refresh - don't fail the request if it fails
                    let _ = self.refresh_token().await;
                }
            }
        }

        // Make the request
        let response = next.run(req, extensions).await?;

        // If we get a 401, try to refresh and suggest re-authentication
        if response.status() == StatusCode::UNAUTHORIZED {
            // Token is invalid - user needs to re-authenticate
            tracing::warn!("Received 401 Unauthorized - credentials may be invalid");
        }

        Ok(response)
    }
}
