//! OAuth provider implementations and configuration.
//!
//! This module provides configuration and utilities for supported OAuth providers.
//! Each provider has its own submodule with specific endpoints and scopes.
//!
//! Supported providers:
//! - GitHub (`github`) - Uses `read:user` and `user:email` scopes
//! - Google (`google`) - Uses `openid`, `email`, and `profile` scopes

pub mod github;
pub mod google;

use oauth2::{basic::BasicClient, AuthUrl, ClientId, DeviceAuthorizationUrl, Scope, TokenUrl};

use crate::auth::tokens::{ApiCredentials, AuthProvider};
use crate::error::Result;

/// Configuration for an OAuth provider.
///
/// Contains all the URLs and settings needed to perform OAuth authentication
/// using the device authorization flow.
pub struct OAuthProviderConfig {
    /// OAuth client ID (public identifier for the application).
    pub client_id: ClientId,
    /// URL for the authorization endpoint.
    pub auth_url: AuthUrl,
    /// URL for the token endpoint.
    pub token_url: TokenUrl,
    /// URL for device authorization (RFC 8628).
    pub device_auth_url: DeviceAuthorizationUrl,
    /// OAuth scopes to request.
    pub scopes: Vec<Scope>,
}

/// Returns the OAuth configuration for a provider.
///
/// # Arguments
///
/// * `provider` - The provider to get configuration for
/// * `custom_client_id` - Optional custom client ID (overrides default)
///
/// # Errors
///
/// Returns [`crate::error::DeskError::Config`] if the provider configuration is invalid.
pub fn get_provider_config(
    provider: AuthProvider,
    custom_client_id: Option<&str>,
) -> Result<OAuthProviderConfig> {
    match provider {
        AuthProvider::GitHub => github::get_config(custom_client_id),
        AuthProvider::Google => google::get_config(custom_client_id),
    }
}

/// Builds an `OAuth2` client from provider configuration.
///
/// Creates a client configured for the device authorization flow
/// (no client secret required).
#[must_use]
pub fn build_client(config: &OAuthProviderConfig) -> BasicClient {
    BasicClient::new(
        config.client_id.clone(),
        None,
        config.auth_url.clone(),
        Some(config.token_url.clone()),
    )
    .set_device_authorization_url(config.device_auth_url.clone())
}

/// Revokes tokens for the given credentials with the OAuth provider.
///
/// This is a best-effort operation that attempts to invalidate the access token
/// with the provider. Revocation may fail silently for device flow tokens.
///
/// # Arguments
///
/// * `credentials` - The credentials containing tokens to revoke
/// * `client_id` - Optional custom client ID used during authentication
///
/// # Returns
///
/// Returns `Ok(true)` if revocation succeeded, `Ok(false)` if it failed gracefully.
pub async fn revoke_credentials(
    credentials: &ApiCredentials,
    client_id: Option<&str>,
) -> Result<bool> {
    let access_token = &credentials.provider_tokens.access_token;

    match credentials.provider {
        AuthProvider::GitHub => {
            let config = get_provider_config(AuthProvider::GitHub, client_id)?;
            github::revoke_token(access_token, config.client_id.as_str()).await
        },
        AuthProvider::Google => google::revoke_token(access_token).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_config_has_correct_urls() {
        let config = get_provider_config(AuthProvider::GitHub, None).expect("config");

        assert_eq!(
            config.auth_url.as_str(),
            "https://github.com/login/oauth/authorize"
        );
        assert_eq!(
            config.token_url.as_str(),
            "https://github.com/login/oauth/access_token"
        );
        assert_eq!(
            config.device_auth_url.as_str(),
            "https://github.com/login/device/code"
        );
    }

    #[test]
    fn github_config_has_required_scopes() {
        let config = get_provider_config(AuthProvider::GitHub, None).expect("config");

        let scope_strs: Vec<_> = config.scopes.iter().map(|s| s.to_string()).collect();
        assert!(scope_strs.contains(&"read:user".to_string()));
        assert!(scope_strs.contains(&"user:email".to_string()));
    }

    #[test]
    fn github_config_accepts_custom_client_id() {
        let custom_id = "custom_client_id_123";
        let config = get_provider_config(AuthProvider::GitHub, Some(custom_id)).expect("config");

        assert_eq!(config.client_id.as_str(), custom_id);
    }

    #[test]
    fn google_config_has_correct_urls() {
        let config = get_provider_config(AuthProvider::Google, None).expect("config");

        assert_eq!(
            config.auth_url.as_str(),
            "https://accounts.google.com/o/oauth2/v2/auth"
        );
        assert_eq!(
            config.token_url.as_str(),
            "https://oauth2.googleapis.com/token"
        );
        assert_eq!(
            config.device_auth_url.as_str(),
            "https://oauth2.googleapis.com/device/code"
        );
    }

    #[test]
    fn google_config_has_required_scopes() {
        let config = get_provider_config(AuthProvider::Google, None).expect("config");

        let scope_strs: Vec<_> = config.scopes.iter().map(|s| s.to_string()).collect();
        assert!(scope_strs.contains(&"openid".to_string()));
        assert!(scope_strs.contains(&"email".to_string()));
        assert!(scope_strs.contains(&"profile".to_string()));
    }

    #[test]
    fn google_config_accepts_custom_client_id() {
        let custom_id = "custom_google_id.apps.googleusercontent.com";
        let config = get_provider_config(AuthProvider::Google, Some(custom_id)).expect("config");

        assert_eq!(config.client_id.as_str(), custom_id);
    }

    #[test]
    fn build_client_creates_valid_client() {
        let config = get_provider_config(AuthProvider::GitHub, None).expect("config");
        let _client = build_client(&config);
    }
}
