//! Google OAuth provider configuration.
//!
//! Configures OAuth endpoints and scopes for Google authentication.
//! Uses the device authorization flow for CLI authentication with
//! `OpenID` Connect scopes for user identity.

use oauth2::{AuthUrl, ClientId, DeviceAuthorizationUrl, Scope, TokenUrl};

use super::OAuthProviderConfig;
use crate::error::{DeskError, Result};

/// Default Google OAuth client ID for desk-cli.
///
/// Replace with your actual Google Cloud Console OAuth client ID
/// for production use.
const DEFAULT_CLIENT_ID: &str = "XXXXXXXXXXXXX.apps.googleusercontent.com";

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const DEVICE_AUTH_URL: &str = "https://oauth2.googleapis.com/device/code";
const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";

/// Returns the Google OAuth provider configuration.
///
/// # Arguments
///
/// * `custom_client_id` - Optional custom client ID (for Google Workspace setups)
///
/// # Errors
///
/// Returns [`DeskError::Config`] if URL parsing fails (should not happen with constants).
pub fn get_config(custom_client_id: Option<&str>) -> Result<OAuthProviderConfig> {
    let client_id = custom_client_id.unwrap_or(DEFAULT_CLIENT_ID);

    Ok(OAuthProviderConfig {
        client_id: ClientId::new(client_id.to_string()),
        auth_url: AuthUrl::new(AUTH_URL.to_string())
            .map_err(|e| DeskError::Config(format!("Invalid auth URL: {e}")))?,
        token_url: TokenUrl::new(TOKEN_URL.to_string())
            .map_err(|e| DeskError::Config(format!("Invalid token URL: {e}")))?,
        device_auth_url: DeviceAuthorizationUrl::new(DEVICE_AUTH_URL.to_string())
            .map_err(|e| DeskError::Config(format!("Invalid device auth URL: {e}")))?,
        scopes: vec![
            Scope::new("openid".to_string()),
            Scope::new("email".to_string()),
            Scope::new("profile".to_string()),
        ],
    })
}

/// Attempts to revoke a Google access or refresh token.
///
/// Google's revocation endpoint accepts either access tokens or refresh tokens
/// and works without client authentication. A 400 response indicates the token
/// was already invalid or revoked, which is treated as success.
///
/// # Arguments
///
/// * `token` - The access token or refresh token to revoke
///
/// # Returns
///
/// Returns `Ok(true)` if revocation succeeded or token was already invalid,
/// `Ok(false)` if revocation failed gracefully.
pub async fn revoke_token(token: &str) -> Result<bool> {
    let client = reqwest::Client::new();

    let response = client
        .post(REVOKE_URL)
        .form(&[("token", token)])
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => Ok(true),
        Ok(resp) if resp.status() == 400 => {
            tracing::debug!("Google token already revoked or invalid");
            Ok(true)
        },
        Ok(resp) => {
            tracing::debug!("Google token revocation returned status {}", resp.status());
            Ok(false)
        },
        Err(e) => {
            tracing::debug!("Google token revocation failed: {e}");
            Ok(false)
        },
    }
}
