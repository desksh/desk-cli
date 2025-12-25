//! GitHub OAuth provider configuration.
//!
//! Configures OAuth endpoints and scopes for GitHub authentication.
//! Uses the device authorization flow for CLI authentication.

use oauth2::{AuthUrl, ClientId, DeviceAuthorizationUrl, Scope, TokenUrl};

use super::OAuthProviderConfig;
use crate::error::{DeskError, Result};

/// Default GitHub OAuth client ID for desk-cli.
///
/// Replace with your actual GitHub OAuth App client ID for production use.
const DEFAULT_CLIENT_ID: &str = "Ov23liXXXXXXXXXXXXXX";

const AUTH_URL: &str = "https://github.com/login/oauth/authorize";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const DEVICE_AUTH_URL: &str = "https://github.com/login/device/code";
const REVOKE_URL: &str = "https://api.github.com/applications";

/// Returns the GitHub OAuth provider configuration.
///
/// # Arguments
///
/// * `custom_client_id` - Optional custom client ID (for enterprise GitHub setups)
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
            Scope::new("read:user".to_string()),
            Scope::new("user:email".to_string()),
        ],
    })
}

/// Attempts to revoke a GitHub access token.
///
/// GitHub token revocation requires Basic Auth with `client_id:client_secret`.
/// Since device flow doesn't require a client secret, this will typically fail
/// with a 401 response. The function handles this gracefully.
///
/// # Arguments
///
/// * `access_token` - The access token to revoke
/// * `client_id` - The OAuth client ID
///
/// # Returns
///
/// Returns `Ok(true)` if revocation succeeded, `Ok(false)` if it failed gracefully.
pub async fn revoke_token(access_token: &str, client_id: &str) -> Result<bool> {
    let client = reqwest::Client::new();
    let url = format!("{REVOKE_URL}/{client_id}/token");

    let response = client
        .delete(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&serde_json::json!({ "access_token": access_token }))
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => Ok(true),
        Ok(resp) => {
            tracing::debug!(
                "GitHub token revocation returned {}: expected for device flow tokens",
                resp.status()
            );
            Ok(false)
        },
        Err(e) => {
            tracing::debug!("GitHub token revocation request failed: {e}");
            Ok(false)
        },
    }
}
