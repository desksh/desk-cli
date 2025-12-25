//! Workspace-specific error types.
//!
//! This module defines error types for workspace operations:
//! - [`WorkspaceError`] - All workspace-related errors with user-friendly messages

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_not_found_returns_true_for_not_found() {
        let err = WorkspaceError::NotFound("test".to_string());
        assert!(err.is_not_found());
    }

    #[test]
    fn is_not_found_returns_false_for_other_errors() {
        assert!(!WorkspaceError::AlreadyExists("test".to_string()).is_not_found());
        assert!(!WorkspaceError::InvalidName("a".to_string(), "b".to_string()).is_not_found());
        assert!(!WorkspaceError::Storage("err".to_string()).is_not_found());
        assert!(!WorkspaceError::Corrupted("err".to_string()).is_not_found());
    }

    #[test]
    fn error_messages_are_user_friendly() {
        let already_exists = WorkspaceError::AlreadyExists("my-ws".to_string());
        assert!(already_exists.to_string().contains("my-ws"));
        assert!(already_exists.to_string().contains("--force"));

        let not_found = WorkspaceError::NotFound("missing".to_string());
        assert!(not_found.to_string().contains("missing"));

        let invalid = WorkspaceError::InvalidName("bad/name".to_string(), "has slash".to_string());
        assert!(invalid.to_string().contains("bad/name"));
        assert!(invalid.to_string().contains("has slash"));
    }
}
