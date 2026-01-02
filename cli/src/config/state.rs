//! State management for desk-cli.
//!
//! Tracks the current workspace per repository.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::paths::{ensure_data_dir, state_file};
use crate::error::Result;

/// Global state tracking current workspace per repository.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeskState {
    /// Map of repository path to current workspace name.
    #[serde(default)]
    pub current_workspaces: HashMap<String, String>,
}

impl DeskState {
    /// Load state from disk, or return default if not found.
    pub fn load() -> Result<Self> {
        let path = state_file()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)?;
        let state: Self = serde_json::from_str(&contents).unwrap_or_default();

        Ok(state)
    }

    /// Save state to disk.
    pub fn save(&self) -> Result<()> {
        ensure_data_dir()?;
        let path = state_file()?;
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json)?;
        Ok(())
    }

    /// Get the current workspace for a repository.
    pub fn get_current(&self, repo_path: &PathBuf) -> Option<&String> {
        let key = repo_path.to_string_lossy().to_string();
        self.current_workspaces.get(&key)
    }

    /// Set the current workspace for a repository.
    pub fn set_current(&mut self, repo_path: &PathBuf, workspace_name: &str) {
        let key = repo_path.to_string_lossy().to_string();
        self.current_workspaces
            .insert(key, workspace_name.to_string());
    }

    /// Clear the current workspace for a repository.
    pub fn clear_current(&mut self, repo_path: &PathBuf) {
        let key = repo_path.to_string_lossy().to_string();
        self.current_workspaces.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_has_no_workspaces() {
        let state = DeskState::default();
        assert!(state.current_workspaces.is_empty());
    }

    #[test]
    fn set_and_get_current_workspace() {
        let mut state = DeskState::default();
        let repo = PathBuf::from("/home/user/project");

        state.set_current(&repo, "feature-auth");

        assert_eq!(state.get_current(&repo), Some(&"feature-auth".to_string()));
    }

    #[test]
    fn clear_current_workspace() {
        let mut state = DeskState::default();
        let repo = PathBuf::from("/home/user/project");

        state.set_current(&repo, "feature-auth");
        state.clear_current(&repo);

        assert_eq!(state.get_current(&repo), None);
    }

    #[test]
    fn serialization_roundtrip() {
        let mut state = DeskState::default();
        state.set_current(&PathBuf::from("/repo1"), "ws1");
        state.set_current(&PathBuf::from("/repo2"), "ws2");

        let json = serde_json::to_string(&state).unwrap();
        let restored: DeskState = serde_json::from_str(&json).unwrap();

        assert_eq!(
            restored.get_current(&PathBuf::from("/repo1")),
            Some(&"ws1".to_string())
        );
        assert_eq!(
            restored.get_current(&PathBuf::from("/repo2")),
            Some(&"ws2".to_string())
        );
    }
}
