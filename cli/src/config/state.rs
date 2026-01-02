//! State management for desk-cli.
//!
//! Tracks the current workspace per repository and switch history.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::paths::{ensure_data_dir, state_file};
use crate::error::Result;

/// Maximum number of history entries to keep.
const MAX_HISTORY_ENTRIES: usize = 50;

/// A history entry recording a workspace switch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Workspace name that was opened.
    pub workspace: String,
    /// Repository path.
    pub repo_path: String,
    /// When the switch occurred.
    pub timestamp: DateTime<Utc>,
}

/// Global state tracking current workspace per repository.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeskState {
    /// Map of repository path to current workspace name.
    #[serde(default)]
    pub current_workspaces: HashMap<String, String>,

    /// History of workspace switches (most recent first).
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
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
            .insert(key.clone(), workspace_name.to_string());

        // Add to history
        self.add_history_entry(workspace_name, &key);
    }

    /// Clear the current workspace for a repository.
    pub fn clear_current(&mut self, repo_path: &PathBuf) {
        let key = repo_path.to_string_lossy().to_string();
        self.current_workspaces.remove(&key);
    }

    /// Add a history entry.
    fn add_history_entry(&mut self, workspace: &str, repo_path: &str) {
        let entry = HistoryEntry {
            workspace: workspace.to_string(),
            repo_path: repo_path.to_string(),
            timestamp: Utc::now(),
        };

        // Insert at the beginning
        self.history.insert(0, entry);

        // Trim to max size
        if self.history.len() > MAX_HISTORY_ENTRIES {
            self.history.truncate(MAX_HISTORY_ENTRIES);
        }
    }

    /// Get recent history entries.
    pub fn get_history(&self, limit: usize) -> &[HistoryEntry] {
        let end = limit.min(self.history.len());
        &self.history[..end]
    }

    /// Get history filtered by repository path.
    pub fn get_history_for_repo(&self, repo_path: &PathBuf, limit: usize) -> Vec<&HistoryEntry> {
        let key = repo_path.to_string_lossy().to_string();
        self.history
            .iter()
            .filter(|e| e.repo_path == key)
            .take(limit)
            .collect()
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
