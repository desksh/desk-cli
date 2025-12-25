//! Workspace data types for desk-cli.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents a saved workspace state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    /// Unique workspace name (used as filename).
    pub name: String,

    /// Path to the repository root.
    pub repo_path: PathBuf,

    /// Git branch at time of save.
    pub branch: String,

    /// Git commit SHA at time of save.
    pub commit_sha: String,

    /// Name of the stash created (if any).
    pub stash_name: Option<String>,

    /// When the workspace was created.
    pub created_at: DateTime<Utc>,

    /// When the workspace was last updated.
    pub updated_at: DateTime<Utc>,

    /// Optional description from user.
    pub description: Option<String>,

    /// Additional metadata (extensible).
    #[serde(default)]
    pub metadata: WorkspaceMetadata,
}

/// Additional extensible metadata for workspaces.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceMetadata {
    /// Number of uncommitted files at save time.
    pub uncommitted_files: Option<u32>,

    /// Whether the working directory was dirty.
    pub was_dirty: Option<bool>,
}

#[allow(dead_code)]
impl Workspace {
    /// Creates a new workspace with the current timestamp.
    #[must_use]
    pub fn new(name: String, repo_path: PathBuf, branch: String, commit_sha: String) -> Self {
        let now = Utc::now();
        Self {
            name,
            repo_path,
            branch,
            commit_sha,
            stash_name: None,
            created_at: now,
            updated_at: now,
            description: None,
            metadata: WorkspaceMetadata::default(),
        }
    }

    /// Updates the `updated_at` timestamp.
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_workspace_has_matching_timestamps() {
        let ws = Workspace::new(
            "test".to_string(),
            PathBuf::from("/repo"),
            "main".to_string(),
            "abc123".to_string(),
        );

        assert_eq!(ws.name, "test");
        assert_eq!(ws.branch, "main");
        assert_eq!(ws.created_at, ws.updated_at);
        assert!(ws.stash_name.is_none());
        assert!(ws.description.is_none());
    }

    #[test]
    fn touch_updates_timestamp() {
        let mut ws = Workspace::new(
            "test".to_string(),
            PathBuf::from("/repo"),
            "main".to_string(),
            "abc123".to_string(),
        );

        let original = ws.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        ws.touch();

        assert!(ws.updated_at > original);
        assert_eq!(ws.created_at, original);
    }

    #[test]
    fn serialization_roundtrip() {
        let ws = Workspace::new(
            "test-ws".to_string(),
            PathBuf::from("/home/user/project"),
            "feature/auth".to_string(),
            "abc123def456".to_string(),
        );

        let json = serde_json::to_string(&ws).unwrap();
        let deserialized: Workspace = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, ws.name);
        assert_eq!(deserialized.repo_path, ws.repo_path);
        assert_eq!(deserialized.branch, ws.branch);
    }
}
