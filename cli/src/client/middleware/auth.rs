//! Authentication middleware for injecting Bearer tokens.

use std::sync::Arc;

use async_trait::async_trait;
use http::Extensions;
use reqwest::{Request, Response};
use reqwest_middleware::{Middleware, Next, Result};
use tokio::sync::RwLock;

use crate::auth::ApiCredentials;

/// Middleware that injects the Bearer token into requests.
pub struct AuthMiddleware {
    credentials: Arc<RwLock<Option<ApiCredentials>>>,
}

impl AuthMiddleware {
    /// Create a new authentication middleware.
    #[must_use]
    pub fn new(credentials: Arc<RwLock<Option<ApiCredentials>>>) -> Self {
        Self { credentials }
    }
}

#[async_trait]
impl Middleware for AuthMiddleware {
    async fn handle(
        &self,
        mut req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> Result<Response> {
        // Inject Authorization header if we have credentials
        if let Some(creds) = self.credentials.read().await.as_ref() {
            if let Ok(value) = format!("Bearer {}", creds.api_token).parse() {
                req.headers_mut().insert(http::header::AUTHORIZATION, value);
            }
        }

        next.run(req, extensions).await
    }
}
