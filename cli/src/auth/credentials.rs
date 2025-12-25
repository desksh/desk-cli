//! Secure credential storage using the operating system keyring.
//!
//! This module provides platform-specific secure storage for API credentials:
//! - macOS: Keychain
//! - Linux: Secret Service (GNOME Keyring, `KWallet`)
//! - Windows: Credential Manager
//!
//! Credentials are stored as JSON in the keyring under a service-specific key.

use keyring::Entry;

use crate::auth::tokens::ApiCredentials;
use crate::error::{DeskError, Result};

const SERVICE_NAME: &str = "dev.getdesk.cli";
const CREDENTIALS_KEY: &str = "api_credentials";

/// Secure credential storage backed by the OS keyring.
///
/// Provides methods to save, load, and delete API credentials using
/// platform-native secure storage mechanisms.
pub struct CredentialStore {
    entry: Entry,
}

impl CredentialStore {
    /// Creates a new credential store instance.
    ///
    /// Initializes connection to the OS keyring for the desk-cli service.
    ///
    /// # Errors
    ///
    /// Returns [`DeskError::CredentialStorage`] if the keyring entry cannot be created,
    /// which may occur if the keyring service is unavailable or locked.
    pub fn new() -> Result<Self> {
        let entry = Entry::new(SERVICE_NAME, CREDENTIALS_KEY)
            .map_err(|e| DeskError::CredentialStorage(e.to_string()))?;
        Ok(Self { entry })
    }

    /// Saves credentials to secure storage.
    ///
    /// Serializes the credentials to JSON and stores them in the OS keyring.
    /// Overwrites any previously stored credentials.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails or the keyring is inaccessible.
    pub fn save(&self, creds: &ApiCredentials) -> Result<()> {
        let json = serde_json::to_string(creds)?;
        self.entry
            .set_password(&json)
            .map_err(|e| DeskError::CredentialStorage(e.to_string()))?;
        Ok(())
    }

    /// Loads credentials from secure storage.
    ///
    /// Returns `None` if no credentials are stored.
    ///
    /// # Errors
    ///
    /// Returns [`DeskError::InvalidCredentials`] if stored data cannot be parsed,
    /// or [`DeskError::CredentialStorage`] if the keyring is inaccessible.
    pub fn load(&self) -> Result<Option<ApiCredentials>> {
        match self.entry.get_password() {
            Ok(json) => {
                let creds: ApiCredentials =
                    serde_json::from_str(&json).map_err(|_| DeskError::InvalidCredentials)?;
                Ok(Some(creds))
            },
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(DeskError::CredentialStorage(e.to_string())),
        }
    }

    /// Deletes stored credentials from the keyring.
    ///
    /// No-op if no credentials are stored. Always succeeds unless
    /// the keyring is inaccessible.
    ///
    /// # Errors
    ///
    /// Returns [`DeskError::CredentialStorage`] if the keyring is inaccessible.
    pub fn delete(&self) -> Result<()> {
        match self.entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(DeskError::CredentialStorage(e.to_string())),
        }
    }
}
