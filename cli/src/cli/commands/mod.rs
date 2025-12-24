//! Command implementations.

pub mod auth;

pub use auth::{handle_login, handle_logout, handle_status};
