//! Workspace-specific error types.

use thiserror::Error;

/// Errors specific to workspace operations.
#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum WorkspaceError {
    /// Workspace with the given name already exists.
    #[error("Workspace '{0}' already exists. Use --force to overwrite.")]
    AlreadyExists(String),

    /// Workspace with the given name was not found.
    #[error("Workspace '{0}' not found.")]
    NotFound(String),

    /// Invalid workspace name (contains invalid characters).
    #[error("Invalid workspace name '{0}': {1}")]
    InvalidName(String, String),

    /// Failed to read/write workspace data.
    #[error("Workspace storage error: {0}")]
    Storage(String),

    /// Failed to serialize/deserialize workspace.
    #[error("Workspace data corrupted: {0}")]
    Corrupted(String),
}

#[allow(dead_code)]
impl WorkspaceError {
    /// Checks if this is a "not found" error that might be recoverable.
    #[must_use]
    pub const fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound(_))
    }
}
