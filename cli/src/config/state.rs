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

    /// Workspace aliases (short name -> full workspace name).
    #[serde(default)]
    pub aliases: HashMap<String, String>,

    /// Pre-switch hooks (commands to run before switching workspaces).
    #[serde(default)]
    pub pre_switch_hooks: Vec<String>,

    /// Post-switch hooks (commands to run after switching workspaces).
    #[serde(default)]
    pub post_switch_hooks: Vec<String>,

    /// When the current workspace was opened (for time tracking).
    #[serde(default)]
    pub current_opened_at: HashMap<String, DateTime<Utc>>,
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

    /// Resolve a name to a workspace name (handling aliases).
    pub fn resolve_alias(&self, name: &str) -> String {
        self.aliases.get(name).cloned().unwrap_or_else(|| name.to_string())
    }

    /// Set an alias for a workspace.
    pub fn set_alias(&mut self, alias: &str, workspace: &str) {
        self.aliases.insert(alias.to_string(), workspace.to_string());
    }

    /// Remove an alias.
    pub fn remove_alias(&mut self, alias: &str) -> bool {
        self.aliases.remove(alias).is_some()
    }

    /// Get all aliases.
    pub fn get_aliases(&self) -> &HashMap<String, String> {
        &self.aliases
    }

    /// Add a pre-switch hook.
    pub fn add_pre_switch_hook(&mut self, command: String) {
        self.pre_switch_hooks.push(command);
    }

    /// Add a post-switch hook.
    pub fn add_post_switch_hook(&mut self, command: String) {
        self.post_switch_hooks.push(command);
    }

    /// Remove a pre-switch hook by index.
    pub fn remove_pre_switch_hook(&mut self, index: usize) -> bool {
        if index < self.pre_switch_hooks.len() {
            self.pre_switch_hooks.remove(index);
            true
        } else {
            false
        }
    }

    /// Remove a post-switch hook by index.
    pub fn remove_post_switch_hook(&mut self, index: usize) -> bool {
        if index < self.post_switch_hooks.len() {
            self.post_switch_hooks.remove(index);
            true
        } else {
            false
        }
    }

    /// Clear all hooks.
    pub fn clear_hooks(&mut self) {
        self.pre_switch_hooks.clear();
        self.post_switch_hooks.clear();
    }

    /// Record when a workspace was opened (for time tracking).
    pub fn record_workspace_opened(&mut self, repo_path: &PathBuf) {
        let key = repo_path.to_string_lossy().to_string();
        self.current_opened_at.insert(key, Utc::now());
    }

    /// Get the duration since the current workspace was opened.
    pub fn get_time_in_workspace(&self, repo_path: &PathBuf) -> Option<u64> {
        let key = repo_path.to_string_lossy().to_string();
        self.current_opened_at.get(&key).map(|opened_at| {
            let duration = Utc::now().signed_duration_since(*opened_at);
            duration.num_seconds().max(0) as u64
        })
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
