//! Configuration management for desk-cli.

pub mod paths;
pub mod settings;

pub use paths::config_file;
pub use settings::{ApiConfig, DeskConfig};

use std::path::Path;

use crate::error::{DeskError, Result};

/// Load configuration from the default config file.
///
/// If the config file doesn't exist, returns default configuration.
pub fn load_config() -> Result<DeskConfig> {
    let path = config_file()?;
    load_config_from(&path)
}

/// Load configuration from a specific path.
///
/// If the file doesn't exist, returns default configuration.
pub fn load_config_from(path: &Path) -> Result<DeskConfig> {
    if !path.exists() {
        return Ok(DeskConfig::default().with_env_overrides());
    }

    let contents = std::fs::read_to_string(path)?;
    let config: DeskConfig =
        toml::from_str(&contents).map_err(|e| DeskError::ConfigRead(e.to_string()))?;

    Ok(config.with_env_overrides())
}

/// Save configuration to the default config file.
#[allow(dead_code)]
pub fn save_config(config: &DeskConfig) -> Result<()> {
    let path = config_file()?;
    save_config_to(config, &path)
}

/// Save configuration to a specific path.
#[allow(dead_code)]
pub fn save_config_to(config: &DeskConfig, path: &Path) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let contents =
        toml::to_string_pretty(config).map_err(|e| DeskError::ConfigWrite(e.to_string()))?;
    std::fs::write(path, contents)?;

    Ok(())
}
