//! Git operations module for desk-cli.
//!
//! Provides an abstraction layer over git operations needed for
//! workspace context switching:
//! - Branch management
//! - Stash operations
//! - Repository status

pub mod error;
pub mod operations;
pub mod types;

#[allow(unused_imports)]
pub use error::GitError;
#[allow(unused_imports)]
pub use operations::{Git2Operations, GitOperations};
#[allow(unused_imports)]
pub use types::{RepoStatus, StashEntry, StashOptions, SwitchOptions};

#[cfg(test)]
#[allow(unused_imports)]
pub use operations::MockGitOperations;
