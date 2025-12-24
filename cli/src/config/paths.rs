//! Platform-specific path utilities for desk-cli.

use std::path::PathBuf;

use crate::error::{DeskError, Result};

/// Get the configuration directory for desk-cli.
///
/// - Linux: `~/.config/desk`
/// - macOS: `~/Library/Application Support/desk`
/// - Windows: `%APPDATA%\desk`
pub fn config_dir() -> Result<PathBuf> {
    let base = dirs::config_dir()
        .ok_or_else(|| DeskError::Config("Cannot determine config directory".to_string()))?;
    Ok(base.join("desk"))
}

/// Get the data directory for desk-cli.
///
/// - Linux: `~/.local/share/desk`
/// - macOS: `~/Library/Application Support/desk`
/// - Windows: `%APPDATA%\desk`
#[allow(dead_code)]
pub fn data_dir() -> Result<PathBuf> {
    let base = dirs::data_dir()
        .ok_or_else(|| DeskError::Config("Cannot determine data directory".to_string()))?;
    Ok(base.join("desk"))
}

/// Get the main configuration file path.
pub fn config_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

/// Get the workspaces directory.
#[allow(dead_code)]
pub fn workspaces_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("workspaces"))
}

/// Ensure the configuration directory exists.
#[allow(dead_code)]
pub fn ensure_config_dir() -> Result<PathBuf> {
    let dir = config_dir()?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

/// Ensure the data directory exists.
#[allow(dead_code)]
pub fn ensure_data_dir() -> Result<PathBuf> {
    let dir = data_dir()?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}
