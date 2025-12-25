//! Git-related types for desk-cli.
//!
//! This module defines data structures for git operations:
//! - [`RepoStatus`] - Current repository state (branch, changes, etc.)
//! - [`StashEntry`] - A git stash entry
//! - [`StashOptions`] - Options for creating stashes
//! - [`SwitchOptions`] - Options for switching branches

use serde::{Deserialize, Serialize};

/// Status of a git repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoStatus {
    /// Current branch name.
    pub branch: String,

    /// Current commit SHA.
    pub commit_sha: String,

    /// Whether the working directory has uncommitted changes.
    pub is_dirty: bool,

    /// Number of staged files.
    pub staged_count: usize,

    /// Number of modified (unstaged) files.
    pub modified_count: usize,

    /// Number of untracked files.
    pub untracked_count: usize,
}

#[allow(dead_code)]
impl RepoStatus {
    /// Returns true if there are any changes (staged, modified, or untracked).
    #[must_use]
    pub const fn has_changes(&self) -> bool {
        self.staged_count > 0 || self.modified_count > 0 || self.untracked_count > 0
    }

    /// Returns the total count of changed files.
    #[must_use]
    pub const fn total_changes(&self) -> usize {
        self.staged_count + self.modified_count + self.untracked_count
    }
}

/// Represents a git stash entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StashEntry {
    /// Stash index (e.g., 0 for stash@{0}).
    pub index: usize,

    /// Stash message.
    pub message: String,

    /// Branch the stash was created on.
    pub branch: Option<String>,
}

/// Options for stashing changes.
#[derive(Debug, Clone, Default)]
pub struct StashOptions {
    /// Message for the stash.
    pub message: Option<String>,

    /// Include untracked files in stash.
    pub include_untracked: bool,
}

/// Options for switching branches.
#[derive(Debug, Clone, Default)]
pub struct SwitchOptions {
    /// Create the branch if it doesn't exist.
    pub create: bool,

    /// Force switch (discard local changes).
    pub force: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_status_has_changes() {
        let clean = RepoStatus {
            branch: "main".to_string(),
            commit_sha: "abc123".to_string(),
            is_dirty: false,
            staged_count: 0,
            modified_count: 0,
            untracked_count: 0,
        };
        assert!(!clean.has_changes());

        let with_staged = RepoStatus {
            staged_count: 1,
            ..clean.clone()
        };
        assert!(with_staged.has_changes());

        let with_modified = RepoStatus {
            modified_count: 2,
            ..clean.clone()
        };
        assert!(with_modified.has_changes());

        let with_untracked = RepoStatus {
            untracked_count: 3,
            ..clean
        };
        assert!(with_untracked.has_changes());
    }

    #[test]
    fn repo_status_total_changes() {
        let status = RepoStatus {
            branch: "main".to_string(),
            commit_sha: "abc123".to_string(),
            is_dirty: true,
            staged_count: 1,
            modified_count: 2,
            untracked_count: 3,
        };
        assert_eq!(status.total_changes(), 6);
    }

    #[test]
    fn stash_options_default() {
        let opts = StashOptions::default();
        assert!(opts.message.is_none());
        assert!(!opts.include_untracked);
    }

    #[test]
    fn switch_options_default() {
        let opts = SwitchOptions::default();
        assert!(!opts.create);
        assert!(!opts.force);
    }
}
