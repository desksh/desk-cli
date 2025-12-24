//! OAuth provider implementations.

pub mod github;
pub mod google;

use oauth2::{basic::BasicClient, AuthUrl, ClientId, DeviceAuthorizationUrl, Scope, TokenUrl};

use crate::auth::tokens::AuthProvider;
use crate::error::Result;

/// Configuration for an OAuth provider.
pub struct OAuthProviderConfig {
    /// OAuth client ID.
    pub client_id: ClientId,
    /// Authorization URL.
    pub auth_url: AuthUrl,
    /// Token URL.
    pub token_url: TokenUrl,
    /// Device authorization URL for device flow.
    pub device_auth_url: DeviceAuthorizationUrl,
    /// Required scopes.
    pub scopes: Vec<Scope>,
}

/// Get the OAuth configuration for a provider.
///
/// # Errors
///
/// Returns an error if the provider configuration is invalid.
pub fn get_provider_config(
    provider: AuthProvider,
    custom_client_id: Option<&str>,
) -> Result<OAuthProviderConfig> {
    match provider {
        AuthProvider::GitHub => github::get_config(custom_client_id),
        AuthProvider::Google => google::get_config(custom_client_id),
    }
}

/// Build an OAuth2 client from provider configuration.
#[must_use]
pub fn build_client(config: &OAuthProviderConfig) -> BasicClient {
    BasicClient::new(
        config.client_id.clone(),
        None, // No client secret for device flow
        config.auth_url.clone(),
        Some(config.token_url.clone()),
    )
    .set_device_authorization_url(config.device_auth_url.clone())
}
