//! Authentication middleware for injecting Bearer tokens.
//!
//! This middleware automatically adds the `Authorization: Bearer <token>` header
//! to outgoing HTTP requests when credentials are available. It reads from a
//! shared credential store that can be updated by the token refresh middleware.

use std::sync::Arc;

use async_trait::async_trait;
use http::Extensions;
use reqwest::{Request, Response};
use reqwest_middleware::{Middleware, Next, Result};
use tokio::sync::RwLock;

use crate::auth::ApiCredentials;

/// Middleware that injects the Bearer token into requests.
///
/// Reads credentials from a shared `RwLock` and adds the API token as a
/// Bearer authorization header. If no credentials are present or the header
/// value cannot be parsed, the request proceeds without authentication.
pub struct AuthMiddleware {
    /// Shared credential store, synchronized with the token refresh middleware.
    credentials: Arc<RwLock<Option<ApiCredentials>>>,
}

impl AuthMiddleware {
    /// Creates a new authentication middleware.
    ///
    /// # Arguments
    ///
    /// * `credentials` - Shared credential store to read tokens from
    #[must_use]
    pub const fn new(credentials: Arc<RwLock<Option<ApiCredentials>>>) -> Self {
        Self { credentials }
    }
}

#[async_trait]
impl Middleware for AuthMiddleware {
    /// Injects the Bearer token into the request if credentials are available.
    ///
    /// If credentials exist, adds an `Authorization: Bearer <api_token>` header.
    /// Header parsing failures are silently ignored to avoid blocking requests.
    async fn handle(
        &self,
        mut req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> Result<Response> {
        let maybe_token = {
            let guard = self.credentials.read().await;
            guard.as_ref().map(|creds| creds.api_token.clone())
        };

        if let Some(token) = maybe_token {
            if let Ok(value) = format!("Bearer {token}").parse() {
                req.headers_mut().insert(http::header::AUTHORIZATION, value);
            } else {
                tracing::warn!("Failed to parse Bearer token as header value");
            }
        }

        next.run(req, extensions).await
    }
}
