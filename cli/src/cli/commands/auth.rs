//! Authentication command handlers for the desk CLI.
//!
//! Implements the `desk auth` subcommands:
//! - `login` - Authenticate with an OAuth provider
//! - `logout` - Clear stored credentials
//! - `status` - Display current authentication status

use crate::auth::{
    open_browser, poll_for_token, revoke_credentials, start_device_flow, AuthProvider,
    CredentialStore,
};
use crate::client::DeskApiClient;
use crate::config::load_config;
use crate::error::Result;

/// Handles the `desk auth login` command.
///
/// Initiates OAuth device flow authentication with the specified provider,
/// exchanges the tokens with the Desk API, and stores the credentials.
///
/// # Arguments
///
/// * `provider` - The OAuth provider to use (GitHub or Google)
/// * `no_browser` - If true, don't attempt to open the browser automatically
pub async fn handle_login(provider: AuthProvider, no_browser: bool) -> Result<()> {
    println!("Logging in with {provider}...\n");

    let config = load_config()?;
    let custom_client_id = match provider {
        AuthProvider::GitHub => config.auth.providers.github.client_id.as_deref(),
        AuthProvider::Google => config.auth.providers.google.client_id.as_deref(),
    };

    let device_auth = start_device_flow(provider, custom_client_id).await?;

    println!("To authenticate, please visit:\n");
    println!("  {}", device_auth.verification_uri);
    println!("\nAnd enter code: {}\n", device_auth.user_code);

    if !no_browser {
        if open_browser(&device_auth) {
            println!("Browser opened automatically.");
        } else {
            println!("Could not open browser. Please visit the URL manually.");
        }
    }
    println!();

    println!("Waiting for authorization...");
    let tokens = poll_for_token(&device_auth).await?;

    println!("Authorization received!\n");

    println!("Exchanging token with Desk API...");
    let api_client = DeskApiClient::new(&config.api)?;
    let credentials = api_client.exchange_token(provider, &tokens).await?;

    let store = CredentialStore::new()?;
    store.save(&credentials)?;

    println!("\nSuccessfully logged in as user {}!", credentials.user_id);

    Ok(())
}

/// Handles the `desk auth logout` command.
///
/// Attempts to revoke the access token with the provider (best-effort),
/// then deletes the stored credentials from the keyring.
pub async fn handle_logout() -> Result<()> {
    let store = CredentialStore::new()?;

    if let Some(credentials) = store.load()? {
        let config = load_config()?;
        let custom_client_id = match credentials.provider {
            AuthProvider::GitHub => config.auth.providers.github.client_id.as_deref(),
            AuthProvider::Google => config.auth.providers.google.client_id.as_deref(),
        };

        print!("Revoking token with {}... ", credentials.provider);
        match revoke_credentials(&credentials, custom_client_id).await {
            Ok(true) => println!("done."),
            Ok(false) => println!("skipped (token may already be invalid)."),
            Err(e) => {
                println!("failed.");
                tracing::debug!("Token revocation error: {e}");
            },
        }

        store.delete()?;
        println!("Successfully logged out.");
    } else {
        println!("Not currently logged in.");
    }

    Ok(())
}

/// Handles the `desk auth status` command.
///
/// Displays the current authentication status including provider,
/// user ID, and whether the token has expired.
pub async fn handle_status() -> Result<()> {
    let config = load_config()?;
    let api_client = DeskApiClient::new(&config.api)?;

    if api_client.load_credentials().await? {
        if let Some(creds) = api_client.get_credentials().await {
            println!("Logged in\n");
            println!("  Provider:   {}", creds.provider);
            println!("  User ID:    {}", creds.user_id);
            println!("  API Server: {}", config.api.base_url);

            if creds.is_api_token_expired() {
                println!("\n  Warning: API token has expired. Run 'desk auth login' to refresh.");
            }
        }
    } else {
        println!("Not logged in\n");
        println!("Run 'desk auth login' to authenticate.");
    }

    Ok(())
}
