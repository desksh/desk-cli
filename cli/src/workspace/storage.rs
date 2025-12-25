//! Workspace storage operations.
//!
//! This module provides persistence for workspace state:
//! - [`WorkspaceStore`] - Trait for workspace storage operations
//! - [`FileWorkspaceStore`] - JSON file-based storage implementation

use std::fs;
use std::path::PathBuf;

use crate::config::paths::workspaces_dir;
use crate::error::Result;
use crate::workspace::error::WorkspaceError;
use crate::workspace::types::Workspace;

/// Trait for workspace storage operations (enables mocking).
#[allow(dead_code)]
pub trait WorkspaceStore {
    /// Saves a workspace to storage.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace already exists and `force` is false,
    /// or if the storage operation fails.
    fn save(&self, workspace: &Workspace, force: bool) -> Result<()>;

    /// Loads a workspace by name.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage operation fails.
    fn load(&self, name: &str) -> Result<Option<Workspace>>;

    /// Deletes a workspace by name.
    ///
    /// Returns `true` if the workspace was deleted, `false` if it didn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage operation fails.
    fn delete(&self, name: &str) -> Result<bool>;

    /// Lists all saved workspaces.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage operation fails.
    fn list(&self) -> Result<Vec<Workspace>>;

    /// Checks if a workspace exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage operation fails.
    fn exists(&self, name: &str) -> Result<bool>;
}

/// File-based workspace storage implementation.
#[allow(dead_code)]
pub struct FileWorkspaceStore {
    base_dir: PathBuf,
}

#[allow(dead_code)]
impl FileWorkspaceStore {
    /// Creates a new file-based workspace store.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspaces directory cannot be determined or created.
    pub fn new() -> Result<Self> {
        let base_dir = ensure_workspaces_dir()?;
        Ok(Self { base_dir })
    }

    /// Creates a store with a custom directory (for testing).
    #[cfg(test)]
    pub fn with_dir(base_dir: PathBuf) -> Result<Self> {
        if !base_dir.exists() {
            fs::create_dir_all(&base_dir)?;
        }
        Ok(Self { base_dir })
    }

    /// Gets the file path for a workspace.
    fn workspace_path(&self, name: &str) -> PathBuf {
        self.base_dir.join(format!("{name}.json"))
    }

    /// Validates a workspace name.
    fn validate_name(name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(WorkspaceError::InvalidName(
                name.to_string(),
                "name cannot be empty".to_string(),
            )
            .into());
        }

        // Check for path traversal or invalid chars
        if name.contains('/') || name.contains('\\') || name.contains("..") {
            return Err(WorkspaceError::InvalidName(
                name.to_string(),
                "name cannot contain path separators".to_string(),
            )
            .into());
        }

        // Reasonable length limit
        if name.len() > 100 {
            return Err(WorkspaceError::InvalidName(
                name.to_string(),
                "name too long (max 100 characters)".to_string(),
            )
            .into());
        }

        Ok(())
    }
}

impl WorkspaceStore for FileWorkspaceStore {
    fn save(&self, workspace: &Workspace, force: bool) -> Result<()> {
        Self::validate_name(&workspace.name)?;

        let path = self.workspace_path(&workspace.name);

        if path.exists() && !force {
            return Err(WorkspaceError::AlreadyExists(workspace.name.clone()).into());
        }

        let json = serde_json::to_string_pretty(workspace)?;
        fs::write(&path, json)?;

        Ok(())
    }

    fn load(&self, name: &str) -> Result<Option<Workspace>> {
        Self::validate_name(name)?;

        let path = self.workspace_path(name);

        if !path.exists() {
            return Ok(None);
        }

        let contents = fs::read_to_string(&path)?;
        let workspace: Workspace = serde_json::from_str(&contents)
            .map_err(|e| WorkspaceError::Corrupted(e.to_string()))?;

        Ok(Some(workspace))
    }

    fn delete(&self, name: &str) -> Result<bool> {
        Self::validate_name(name)?;

        let path = self.workspace_path(name);

        if !path.exists() {
            return Ok(false);
        }

        fs::remove_file(&path)?;
        Ok(true)
    }

    fn list(&self) -> Result<Vec<Workspace>> {
        let mut workspaces = Vec::new();

        if !self.base_dir.exists() {
            return Ok(workspaces);
        }

        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(contents) = fs::read_to_string(&path) {
                    if let Ok(workspace) = serde_json::from_str::<Workspace>(&contents) {
                        workspaces.push(workspace);
                    }
                }
            }
        }

        // Sort by updated_at descending (most recent first)
        workspaces.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(workspaces)
    }

    fn exists(&self, name: &str) -> Result<bool> {
        Self::validate_name(name)?;
        Ok(self.workspace_path(name).exists())
    }
}

/// Ensure the workspaces directory exists.
#[allow(dead_code)]
fn ensure_workspaces_dir() -> Result<PathBuf> {
    let dir = workspaces_dir()?;
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_workspace(name: &str) -> Workspace {
        Workspace::new(
            name.to_string(),
            PathBuf::from("/test/repo"),
            "main".to_string(),
            "abc123def".to_string(),
        )
    }

    #[test]
    fn save_and_load_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileWorkspaceStore::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let workspace = create_test_workspace("test-workspace");
        store.save(&workspace, false).unwrap();

        let loaded = store.load("test-workspace").unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "test-workspace");
    }

    #[test]
    fn load_nonexistent_returns_none() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileWorkspaceStore::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let loaded = store.load("nonexistent").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn reject_duplicate_workspace_without_force() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileWorkspaceStore::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let workspace = create_test_workspace("dup");
        store.save(&workspace, false).unwrap();

        let result = store.save(&workspace, false);
        assert!(result.is_err());
    }

    #[test]
    fn allow_overwrite_with_force() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileWorkspaceStore::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let workspace = create_test_workspace("force-test");
        store.save(&workspace, false).unwrap();
        store.save(&workspace, true).unwrap(); // Should succeed
    }

    #[test]
    fn delete_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileWorkspaceStore::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let workspace = create_test_workspace("to-delete");
        store.save(&workspace, false).unwrap();

        assert!(store.exists("to-delete").unwrap());
        assert!(store.delete("to-delete").unwrap());
        assert!(!store.exists("to-delete").unwrap());
    }

    #[test]
    fn delete_nonexistent_returns_false() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileWorkspaceStore::with_dir(temp_dir.path().to_path_buf()).unwrap();

        assert!(!store.delete("nonexistent").unwrap());
    }

    #[test]
    fn list_workspaces() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileWorkspaceStore::with_dir(temp_dir.path().to_path_buf()).unwrap();

        store.save(&create_test_workspace("ws1"), false).unwrap();
        store.save(&create_test_workspace("ws2"), false).unwrap();
        store.save(&create_test_workspace("ws3"), false).unwrap();

        let list = store.list().unwrap();
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn list_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileWorkspaceStore::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let list = store.list().unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn reject_empty_name() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileWorkspaceStore::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let result = store.load("");
        assert!(result.is_err());
    }

    #[test]
    fn reject_path_traversal() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileWorkspaceStore::with_dir(temp_dir.path().to_path_buf()).unwrap();

        assert!(store.load("../escape").is_err());
        assert!(store.load("foo/bar").is_err());
        assert!(store.load("foo\\bar").is_err());
    }

    #[test]
    fn reject_long_name() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileWorkspaceStore::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let long_name = "a".repeat(101);
        assert!(store.load(&long_name).is_err());
    }

    #[test]
    fn load_corrupted_json_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileWorkspaceStore::with_dir(temp_dir.path().to_path_buf()).unwrap();

        // Write invalid JSON
        let path = temp_dir.path().join("corrupted.json");
        std::fs::write(&path, "{ invalid json }").unwrap();

        let result = store.load("corrupted");
        assert!(result.is_err());
    }

    #[test]
    fn list_skips_invalid_json_files() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileWorkspaceStore::with_dir(temp_dir.path().to_path_buf()).unwrap();

        // Save a valid workspace
        store.save(&create_test_workspace("valid"), false).unwrap();

        // Write an invalid JSON file
        std::fs::write(temp_dir.path().join("invalid.json"), "not json").unwrap();

        // Write a non-json file (should be skipped)
        std::fs::write(temp_dir.path().join("readme.txt"), "text file").unwrap();

        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "valid");
    }

    #[test]
    fn exists_returns_true_for_existing() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileWorkspaceStore::with_dir(temp_dir.path().to_path_buf()).unwrap();

        store
            .save(&create_test_workspace("exists-test"), false)
            .unwrap();

        assert!(store.exists("exists-test").unwrap());
        assert!(!store.exists("does-not-exist").unwrap());
    }
}
