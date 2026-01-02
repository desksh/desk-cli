//! State management for desk-cli.
//!
//! Tracks the current workspace per repository and switch history.

use std::collections::HashMap;
use std::fs;
#[cfg(test)]
use std::path::PathBuf;
use std::path::Path;

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
    pub fn get_current(&self, repo_path: &Path) -> Option<&String> {
        let key = repo_path.to_string_lossy().to_string();
        self.current_workspaces.get(&key)
    }

    /// Set the current workspace for a repository.
    pub fn set_current(&mut self, repo_path: &Path, workspace_name: &str) {
        let key = repo_path.to_string_lossy().to_string();
        self.current_workspaces
            .insert(key.clone(), workspace_name.to_string());

        // Add to history
        self.add_history_entry(workspace_name, &key);
    }

    /// Clear the current workspace for a repository.
    pub fn clear_current(&mut self, repo_path: &Path) {
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
    pub fn get_history_for_repo(&self, repo_path: &Path, limit: usize) -> Vec<&HistoryEntry> {
        let key = repo_path.to_string_lossy().to_string();
        self.history
            .iter()
            .filter(|e| e.repo_path == key)
            .take(limit)
            .collect()
    }

    /// Resolve a name to a workspace name (handling aliases).
    pub fn resolve_alias(&self, name: &str) -> String {
        self.aliases
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    }

    /// Set an alias for a workspace.
    pub fn set_alias(&mut self, alias: &str, workspace: &str) {
        self.aliases
            .insert(alias.to_string(), workspace.to_string());
    }

    /// Remove an alias.
    pub fn remove_alias(&mut self, alias: &str) -> bool {
        self.aliases.remove(alias).is_some()
    }

    /// Get all aliases.
    pub const fn get_aliases(&self) -> &HashMap<String, String> {
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
    pub fn record_workspace_opened(&mut self, repo_path: &Path) {
        let key = repo_path.to_string_lossy().to_string();
        self.current_opened_at.insert(key, Utc::now());
    }

    /// Get the duration since the current workspace was opened.
    #[allow(dead_code)]
    pub fn get_time_in_workspace(&self, repo_path: &Path) -> Option<u64> {
        let key = repo_path.to_string_lossy().to_string();
        self.current_opened_at.get(&key).map(|opened_at| {
            let duration = Utc::now().signed_duration_since(*opened_at);
            #[allow(clippy::cast_sign_loss)]
            let secs = duration.num_seconds().max(0) as u64;
            secs
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

    #[test]
    fn set_current_adds_to_history() {
        let mut state = DeskState::default();
        let repo = PathBuf::from("/home/user/project");

        state.set_current(&repo, "feature-1");
        state.set_current(&repo, "feature-2");

        assert_eq!(state.history.len(), 2);
        assert_eq!(state.history[0].workspace, "feature-2");
        assert_eq!(state.history[1].workspace, "feature-1");
    }

    #[test]
    fn get_history_respects_limit() {
        let mut state = DeskState::default();
        let repo = PathBuf::from("/repo");

        for i in 0..10 {
            state.set_current(&repo, &format!("ws-{i}"));
        }

        let history = state.get_history(5);
        assert_eq!(history.len(), 5);
        assert_eq!(history[0].workspace, "ws-9");
    }

    #[test]
    fn get_history_for_repo_filters_by_repo() {
        let mut state = DeskState::default();
        let repo1 = PathBuf::from("/repo1");
        let repo2 = PathBuf::from("/repo2");

        state.set_current(&repo1, "ws-a");
        state.set_current(&repo2, "ws-b");
        state.set_current(&repo1, "ws-c");

        let history = state.get_history_for_repo(&repo1, 10);
        assert_eq!(history.len(), 2);
        assert!(history.iter().all(|e| e.repo_path == "/repo1"));
    }

    #[test]
    fn history_truncates_at_max() {
        let mut state = DeskState::default();
        let repo = PathBuf::from("/repo");

        // Add more than MAX_HISTORY_ENTRIES
        for i in 0..60 {
            state.set_current(&repo, &format!("ws-{i}"));
        }

        // Should be capped at MAX_HISTORY_ENTRIES (50)
        assert!(state.history.len() <= 50);
    }

    #[test]
    fn set_and_get_alias() {
        let mut state = DeskState::default();
        state.set_alias("f", "feature-branch");

        assert_eq!(state.resolve_alias("f"), "feature-branch".to_string());
        assert_eq!(state.resolve_alias("unknown"), "unknown".to_string());
    }

    #[test]
    fn remove_alias() {
        let mut state = DeskState::default();
        state.set_alias("f", "feature-branch");

        assert!(state.remove_alias("f"));
        assert!(!state.remove_alias("f")); // Already removed
        assert_eq!(state.resolve_alias("f"), "f".to_string());
    }

    #[test]
    fn get_aliases_returns_all() {
        let mut state = DeskState::default();
        state.set_alias("a", "workspace-a");
        state.set_alias("b", "workspace-b");

        let aliases = state.get_aliases();
        assert_eq!(aliases.len(), 2);
        assert_eq!(aliases.get("a"), Some(&"workspace-a".to_string()));
    }

    #[test]
    fn add_and_remove_pre_switch_hook() {
        let mut state = DeskState::default();
        state.add_pre_switch_hook("echo before".to_string());
        state.add_pre_switch_hook("npm run build".to_string());

        assert_eq!(state.pre_switch_hooks.len(), 2);

        assert!(state.remove_pre_switch_hook(0));
        assert_eq!(state.pre_switch_hooks.len(), 1);
        assert_eq!(state.pre_switch_hooks[0], "npm run build");

        assert!(!state.remove_pre_switch_hook(5)); // Out of bounds
    }

    #[test]
    fn add_and_remove_post_switch_hook() {
        let mut state = DeskState::default();
        state.add_post_switch_hook("echo after".to_string());

        assert_eq!(state.post_switch_hooks.len(), 1);

        assert!(state.remove_post_switch_hook(0));
        assert!(state.post_switch_hooks.is_empty());
    }

    #[test]
    fn clear_hooks() {
        let mut state = DeskState::default();
        state.add_pre_switch_hook("pre".to_string());
        state.add_post_switch_hook("post".to_string());

        state.clear_hooks();

        assert!(state.pre_switch_hooks.is_empty());
        assert!(state.post_switch_hooks.is_empty());
    }

    #[test]
    fn record_workspace_opened() {
        let mut state = DeskState::default();
        let repo = PathBuf::from("/repo");

        state.record_workspace_opened(&repo);

        assert!(state.current_opened_at.contains_key("/repo"));
    }

    #[test]
    fn multiple_repos_tracked_independently() {
        let mut state = DeskState::default();
        let repo1 = PathBuf::from("/repo1");
        let repo2 = PathBuf::from("/repo2");

        state.set_current(&repo1, "ws-1");
        state.set_current(&repo2, "ws-2");

        assert_eq!(state.get_current(&repo1), Some(&"ws-1".to_string()));
        assert_eq!(state.get_current(&repo2), Some(&"ws-2".to_string()));

        state.clear_current(&repo1);
        assert_eq!(state.get_current(&repo1), None);
        assert_eq!(state.get_current(&repo2), Some(&"ws-2".to_string()));
    }
}
