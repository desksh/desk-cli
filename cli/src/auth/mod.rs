//! Authentication module for desk-cli.
//!
//! This module provides OAuth device flow authentication with support for
//! multiple providers (GitHub, Google) and secure credential storage.

pub mod credentials;
pub mod device_flow;
pub mod providers;
pub mod tokens;

pub use credentials::CredentialStore;
pub use device_flow::{open_browser, poll_for_token, start_device_flow};
pub use providers::revoke_credentials;
pub use tokens::{ApiCredentials, AuthProvider, TokenSet};
