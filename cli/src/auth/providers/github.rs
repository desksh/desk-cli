//! GitHub OAuth provider configuration.

use oauth2::{AuthUrl, ClientId, DeviceAuthorizationUrl, Scope, TokenUrl};

use super::OAuthProviderConfig;
use crate::error::{DeskError, Result};

/// Default GitHub OAuth client ID for desk-cli.
/// This should be replaced with your actual GitHub OAuth App client ID.
const DEFAULT_CLIENT_ID: &str = "Ov23liXXXXXXXXXXXXXX";

/// GitHub OAuth endpoints.
const AUTH_URL: &str = "https://github.com/login/oauth/authorize";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const DEVICE_AUTH_URL: &str = "https://github.com/login/device/code";

/// Get the GitHub OAuth provider configuration.
///
/// # Arguments
///
/// * `custom_client_id` - Optional custom client ID (for enterprise setups).
///
/// # Errors
///
/// Returns an error if the URLs cannot be parsed.
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
