//! Git-specific error types.
//!
//! This module defines error types for git operations:
//! - [`GitError`] - All git-related errors with user-friendly messages

use thiserror::Error;

/// Errors specific to git operations.
#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum GitError {
    /// Not in a git repository.
    #[error("Not a git repository. Run 'git init' or navigate to a git repository.")]
    NotARepository,

    /// Repository is in a dirty state when clean is required.
    #[error("Working directory has uncommitted changes. Commit or stash changes first.")]
    DirtyWorkingDirectory,

    /// Branch not found.
    #[error("Branch '{0}' not found.")]
    BranchNotFound(String),

    /// Stash not found.
    #[error("Stash '{0}' not found.")]
    StashNotFound(String),

    /// Conflict during operation.
    #[error("Git operation failed due to conflicts: {0}")]
    Conflict(String),

    /// General git2 library error.
    #[error("Git error: {0}")]
    Git2(String),

    /// Failed to determine repository state.
    #[error("Failed to get repository status: {0}")]
    StatusFailed(String),
}

#[allow(dead_code)]
impl GitError {
    /// Checks if this error indicates a repository is not found.
    #[must_use]
    pub const fn is_not_repository(&self) -> bool {
        matches!(self, Self::NotARepository)
    }

    /// Checks if this error is due to conflicts.
    #[must_use]
    pub const fn is_conflict(&self) -> bool {
        matches!(self, Self::Conflict(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_not_repository_returns_true() {
        assert!(GitError::NotARepository.is_not_repository());
    }

    #[test]
    fn is_not_repository_returns_false_for_other_errors() {
        assert!(!GitError::DirtyWorkingDirectory.is_not_repository());
        assert!(!GitError::BranchNotFound("main".to_string()).is_not_repository());
        assert!(!GitError::Conflict("merge".to_string()).is_not_repository());
    }

    #[test]
    fn is_conflict_returns_true() {
        assert!(GitError::Conflict("merge conflict".to_string()).is_conflict());
    }

    #[test]
    fn is_conflict_returns_false_for_other_errors() {
        assert!(!GitError::NotARepository.is_conflict());
        assert!(!GitError::DirtyWorkingDirectory.is_conflict());
        assert!(!GitError::BranchNotFound("main".to_string()).is_conflict());
    }

    #[test]
    fn error_messages_are_user_friendly() {
        let not_repo = GitError::NotARepository;
        assert!(not_repo.to_string().contains("git init"));

        let dirty = GitError::DirtyWorkingDirectory;
        assert!(dirty.to_string().contains("uncommitted"));

        let branch = GitError::BranchNotFound("feature/test".to_string());
        assert!(branch.to_string().contains("feature/test"));

        let stash = GitError::StashNotFound("my-stash".to_string());
        assert!(stash.to_string().contains("my-stash"));
    }
}
