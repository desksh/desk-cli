//! Workspace data types for desk-cli.
//!
//! This module defines the core data structures for workspace state:
//! - [`Workspace`] - Saved workspace state including git context
//! - [`WorkspaceMetadata`] - Extensible metadata for workspace snapshots

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

    /// Remote workspace ID (set after first sync).
    pub remote_id: Option<String>,

    /// Remote version number for conflict detection.
    pub remote_version: Option<i32>,

    /// When the workspace was last synced with the server.
    pub last_synced_at: Option<DateTime<Utc>>,

    /// Tags for organizing workspaces.
    #[serde(default)]
    pub tags: Vec<String>,
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

    #[test]
    fn metadata_defaults_to_none() {
        let metadata = WorkspaceMetadata::default();

        assert!(metadata.uncommitted_files.is_none());
        assert!(metadata.was_dirty.is_none());
        assert!(metadata.remote_id.is_none());
        assert!(metadata.remote_version.is_none());
        assert!(metadata.last_synced_at.is_none());
    }

    #[test]
    fn workspace_with_sync_metadata_serializes() {
        let mut ws = Workspace::new(
            "synced-ws".to_string(),
            PathBuf::from("/home/user/project"),
            "main".to_string(),
            "abc123".to_string(),
        );

        // Set sync metadata
        ws.metadata.remote_id = Some("remote-uuid-123".to_string());
        ws.metadata.remote_version = Some(5);
        ws.metadata.last_synced_at = Some(Utc::now());
        ws.metadata.uncommitted_files = Some(3);
        ws.metadata.was_dirty = Some(true);

        let json = serde_json::to_string(&ws).unwrap();
        let deserialized: Workspace = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.metadata.remote_id, ws.metadata.remote_id);
        assert_eq!(deserialized.metadata.remote_version, Some(5));
        assert!(deserialized.metadata.last_synced_at.is_some());
        assert_eq!(deserialized.metadata.uncommitted_files, Some(3));
        assert_eq!(deserialized.metadata.was_dirty, Some(true));
    }

    #[test]
    fn old_workspace_format_deserializes_with_defaults() {
        // Simulate an old workspace JSON without sync fields
        let old_json = r#"{
            "name": "old-ws",
            "repo_path": "/home/user/old",
            "branch": "main",
            "commit_sha": "abc123",
            "stash_name": null,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
            "description": null,
            "metadata": {
                "uncommitted_files": 2,
                "was_dirty": true
            }
        }"#;

        let ws: Workspace = serde_json::from_str(old_json).unwrap();

        assert_eq!(ws.name, "old-ws");
        // New sync fields should default to None
        assert!(ws.metadata.remote_id.is_none());
        assert!(ws.metadata.remote_version.is_none());
        assert!(ws.metadata.last_synced_at.is_none());
        // Old fields should be preserved
        assert_eq!(ws.metadata.uncommitted_files, Some(2));
        assert_eq!(ws.metadata.was_dirty, Some(true));
    }
}
