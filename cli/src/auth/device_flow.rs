//! OAuth device authorization flow implementation (RFC 8628).

use std::time::Duration;

use chrono::Utc;
use oauth2::basic::BasicTokenType;
use oauth2::devicecode::StandardDeviceAuthorizationResponse;
use oauth2::reqwest::async_http_client;
use oauth2::{EmptyExtraTokenFields, StandardTokenResponse, TokenResponse};

use crate::auth::providers::{build_client, get_provider_config};
use crate::auth::tokens::{AuthProvider, TokenSet};
use crate::error::{DeskError, Result};

/// Response from initiating device authorization.
#[allow(dead_code)]
pub struct DeviceAuthResponse {
    /// URL for the user to visit.
    pub verification_uri: String,
    /// Complete URL with code (if available).
    pub verification_uri_complete: Option<String>,
    /// Code for the user to enter.
    pub user_code: String,
    /// How long until the code expires.
    pub expires_in: Duration,
    /// Internal response for polling.
    inner: StandardDeviceAuthorizationResponse,
    /// The provider being used.
    provider: AuthProvider,
    /// Custom client ID if any.
    custom_client_id: Option<String>,
}

/// Start the device authorization flow.
///
/// Returns the verification URL and user code for the user to complete authentication.
///
/// # Arguments
///
/// * `provider` - The OAuth provider to use.
/// * `custom_client_id` - Optional custom client ID.
///
/// # Errors
///
/// Returns an error if the device authorization request fails.
pub async fn start_device_flow(
    provider: AuthProvider,
    custom_client_id: Option<&str>,
) -> Result<DeviceAuthResponse> {
    let config = get_provider_config(provider, custom_client_id)?;
    let client = build_client(&config);

    let mut request = client
        .exchange_device_code()
        .map_err(|e| DeskError::AuthenticationFailed(format!("Device flow not supported: {e}")))?;

    for scope in &config.scopes {
        request = request.add_scope(scope.clone());
    }

    let response = request
        .request_async(async_http_client)
        .await
        .map_err(|e| DeskError::AuthenticationFailed(format!("Device authorization failed: {e}")))?;

    Ok(DeviceAuthResponse {
        verification_uri: response.verification_uri().to_string(),
        verification_uri_complete: response
            .verification_uri_complete()
            .map(|u| u.secret().clone()),
        user_code: response.user_code().secret().to_string(),
        expires_in: response.expires_in(),
        inner: response,
        provider,
        custom_client_id: custom_client_id.map(String::from),
    })
}

/// Poll for token completion.
///
/// This will poll the token endpoint until the user completes authorization
/// or the device code expires.
///
/// # Arguments
///
/// * `device_auth` - The device authorization response from `start_device_flow`.
///
/// # Errors
///
/// Returns an error if:
/// - The device code expires
/// - The user denies access
/// - A network error occurs
pub async fn poll_for_token(device_auth: &DeviceAuthResponse) -> Result<TokenSet> {
    let config = get_provider_config(
        device_auth.provider,
        device_auth.custom_client_id.as_deref(),
    )?;
    let client = build_client(&config);

    let token_response: StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType> = client
        .exchange_device_access_token(&device_auth.inner)
        .request_async(async_http_client, tokio::time::sleep, None)
        .await
        .map_err(|e| {
            let error_msg = e.to_string();
            if error_msg.contains("expired") {
                DeskError::DeviceAuthorizationExpired
            } else if error_msg.contains("denied") || error_msg.contains("access_denied") {
                DeskError::AccessDenied
            } else {
                DeskError::AuthenticationFailed(format!("Token exchange failed: {e}"))
            }
        })?;

    // Calculate expiration time
    let expires_at = token_response.expires_in().map(|duration| {
        Utc::now() + chrono::Duration::from_std(duration).unwrap_or_default()
    });

    Ok(TokenSet {
        access_token: token_response.access_token().secret().to_string(),
        refresh_token: token_response.refresh_token().map(|t| t.secret().to_string()),
        token_type: "Bearer".to_string(),
        expires_at,
        scope: token_response
            .scopes()
            .map(|scopes| scopes.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(" ")),
    })
}

/// Open the verification URL in the default browser.
///
/// # Arguments
///
/// * `device_auth` - The device authorization response.
///
/// # Returns
///
/// Returns `true` if the browser was opened successfully, `false` otherwise.
pub fn open_browser(device_auth: &DeviceAuthResponse) -> bool {
    let url = device_auth
        .verification_uri_complete
        .as_ref()
        .unwrap_or(&device_auth.verification_uri);

    open::that(url).is_ok()
}
