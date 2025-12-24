//! Authentication command handlers.

use crate::auth::{
    open_browser, poll_for_token, start_device_flow, AuthProvider, CredentialStore,
};
use crate::client::DeskApiClient;
use crate::config::load_config;
use crate::error::Result;

/// Handle the `desk auth login` command.
pub async fn handle_login(provider: AuthProvider, no_browser: bool) -> Result<()> {
    println!("Logging in with {}...", provider);
    println!();

    // Load configuration
    let config = load_config()?;
    let custom_client_id = match provider {
        AuthProvider::GitHub => config.auth.providers.github.client_id.as_deref(),
        AuthProvider::Google => config.auth.providers.google.client_id.as_deref(),
    };

    // Step 1: Start device flow
    let device_auth = start_device_flow(provider, custom_client_id).await?;

    // Step 2: Display instructions
    println!("To authenticate, please visit:");
    println!();
    println!("  {}", device_auth.verification_uri);
    println!();
    println!("And enter code: {}", device_auth.user_code);
    println!();

    // Step 3: Open browser (unless disabled)
    if !no_browser {
        if open_browser(&device_auth) {
            println!("Browser opened automatically.");
        } else {
            println!("Could not open browser. Please visit the URL manually.");
        }
    }
    println!();

    // Step 4: Poll for token
    println!("Waiting for authorization...");
    let tokens = poll_for_token(&device_auth).await?;

    println!("Authorization received!");
    println!();

    // Step 5: Exchange for API token
    println!("Exchanging token with Desk API...");
    let api_client = DeskApiClient::new(&config.api)?;
    let credentials = api_client.exchange_token(provider, &tokens).await?;

    // Step 6: Store credentials
    let store = CredentialStore::new()?;
    store.save(&credentials)?;

    println!();
    println!("Successfully logged in as user {}!", credentials.user_id);

    Ok(())
}

/// Handle the `desk auth logout` command.
pub async fn handle_logout() -> Result<()> {
    let store = CredentialStore::new()?;

    if store.has_credentials() {
        store.delete()?;
        println!("Successfully logged out.");
    } else {
        println!("Not currently logged in.");
    }

    Ok(())
}

/// Handle the `desk auth status` command.
pub async fn handle_status() -> Result<()> {
    let config = load_config()?;
    let api_client = DeskApiClient::new(&config.api)?;

    if api_client.load_credentials().await? {
        if let Some(creds) = api_client.get_credentials().await {
            println!("Logged in");
            println!();
            println!("  Provider:   {}", creds.provider);
            println!("  User ID:    {}", creds.user_id);
            println!("  API Server: {}", config.api.base_url);

            if creds.is_api_token_expired() {
                println!();
                println!("  Warning: API token has expired. Please run 'desk auth login' again.");
            }
        }
    } else {
        println!("Not logged in");
        println!();
        println!("Run 'desk auth login' to authenticate.");
    }

    Ok(())
}

/// Run an auth subcommand without needing the full API client.
#[allow(dead_code)]
pub async fn run_without_api(provider: AuthProvider, no_browser: bool) -> Result<()> {
    println!("Logging in with {}...", provider);
    println!();

    // Load configuration
    let config = load_config()?;
    let custom_client_id = match provider {
        AuthProvider::GitHub => config.auth.providers.github.client_id.as_deref(),
        AuthProvider::Google => config.auth.providers.google.client_id.as_deref(),
    };

    // Start device flow
    let device_auth = start_device_flow(provider, custom_client_id).await?;

    // Display instructions
    println!("To authenticate, please visit:");
    println!();
    println!("  {}", device_auth.verification_uri);
    println!();
    println!("And enter code: {}", device_auth.user_code);
    println!();

    // Open browser (unless disabled)
    if !no_browser && open_browser(&device_auth) {
        println!("Browser opened automatically.");
        println!();
    }

    // Poll for token
    println!("Waiting for authorization...");
    let tokens = poll_for_token(&device_auth).await?;

    println!("Authorization received!");
    println!();

    // For now, just store the provider tokens directly
    // In a real implementation, we'd exchange these for API tokens
    println!("Note: Backend token exchange not yet implemented.");
    println!("Provider tokens received successfully.");
    println!();
    println!("Access token: {}...", &tokens.access_token[..20.min(tokens.access_token.len())]);

    Ok(())
}
