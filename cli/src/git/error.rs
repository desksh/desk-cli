//! Git-specific error types.

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
