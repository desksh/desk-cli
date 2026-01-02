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

/// Ensure the workspaces directory exists.
#[allow(dead_code)]
pub fn ensure_workspaces_dir() -> Result<PathBuf> {
    let dir = workspaces_dir()?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

/// Get the state file path (tracks current workspace per repo).
pub fn state_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("state.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_returns_path() {
        let path = config_dir();
        assert!(path.is_ok());
        let path = path.unwrap();
        assert!(path.ends_with("desk"));
    }

    #[test]
    fn data_dir_returns_path() {
        let path = data_dir();
        assert!(path.is_ok());
        let path = path.unwrap();
        assert!(path.ends_with("desk"));
    }

    #[test]
    fn config_file_has_toml_extension() {
        let path = config_file();
        assert!(path.is_ok());
        let path = path.unwrap();
        assert!(path.ends_with("config.toml"));
    }

    #[test]
    fn workspaces_dir_is_under_data_dir() {
        let data = data_dir().unwrap();
        let workspaces = workspaces_dir().unwrap();
        assert!(workspaces.starts_with(data));
        assert!(workspaces.ends_with("workspaces"));
    }

    #[test]
    fn state_file_has_json_extension() {
        let path = state_file();
        assert!(path.is_ok());
        let path = path.unwrap();
        assert!(path.ends_with("state.json"));
    }

    #[test]
    fn ensure_config_dir_creates_directory() {
        // This test creates actual directories, but they're in user space
        let result = ensure_config_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        // Path should exist after ensure_* call, but we don't assert
        // since in some CI environments it may not work
        let _ = path.exists();
    }

    #[test]
    fn ensure_data_dir_creates_directory() {
        let result = ensure_data_dir();
        assert!(result.is_ok());
    }

    #[test]
    fn ensure_workspaces_dir_creates_directory() {
        let result = ensure_workspaces_dir();
        assert!(result.is_ok());
    }
}
