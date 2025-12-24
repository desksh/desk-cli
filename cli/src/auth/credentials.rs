//! Secure credential storage using the OS keyring.

use keyring::Entry;

use crate::auth::tokens::ApiCredentials;
use crate::error::{DeskError, Result};

/// Service name for keyring entries.
const SERVICE_NAME: &str = "dev.getdesk.cli";
/// Key for storing API credentials.
const CREDENTIALS_KEY: &str = "api_credentials";

/// Secure credential storage backed by the OS keyring.
///
/// Uses platform-specific secure storage:
/// - macOS: Keychain
/// - Linux: Secret Service (GNOME Keyring, KWallet)
/// - Windows: Credential Manager
pub struct CredentialStore {
    entry: Entry,
}

impl CredentialStore {
    /// Create a new credential store.
    ///
    /// # Errors
    ///
    /// Returns an error if the keyring entry cannot be created.
    pub fn new() -> Result<Self> {
        let entry = Entry::new(SERVICE_NAME, CREDENTIALS_KEY)
            .map_err(|e| DeskError::CredentialStorage(e.to_string()))?;
        Ok(Self { entry })
    }

    /// Save credentials to secure storage.
    ///
    /// # Errors
    ///
    /// Returns an error if the credentials cannot be saved.
    pub fn save(&self, creds: &ApiCredentials) -> Result<()> {
        let json = serde_json::to_string(creds)?;
        self.entry
            .set_password(&json)
            .map_err(|e| DeskError::CredentialStorage(e.to_string()))?;
        Ok(())
    }

    /// Load credentials from secure storage.
    ///
    /// Returns `None` if no credentials are stored.
    ///
    /// # Errors
    ///
    /// Returns an error if the credentials cannot be read or parsed.
    pub fn load(&self) -> Result<Option<ApiCredentials>> {
        match self.entry.get_password() {
            Ok(json) => {
                let creds: ApiCredentials = serde_json::from_str(&json)?;
                Ok(Some(creds))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(DeskError::CredentialStorage(e.to_string())),
        }
    }

    /// Delete stored credentials.
    ///
    /// # Errors
    ///
    /// Returns an error if the credentials cannot be deleted.
    pub fn delete(&self) -> Result<()> {
        match self.entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(DeskError::CredentialStorage(e.to_string())),
        }
    }

    /// Check if credentials are stored.
    #[must_use]
    pub fn has_credentials(&self) -> bool {
        self.entry.get_password().is_ok()
    }
}
