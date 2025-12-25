//! Git operations abstraction for desk-cli.
//!
//! This module provides a trait-based abstraction over git operations:
//! - [`GitOperations`] - Trait defining git operations for workspace switching
//! - [`Git2Operations`] - Implementation using the git2 (libgit2) library

use std::path::Path;

use git2::{Repository, Signature, StashFlags, StatusOptions};

use crate::error::Result;
use crate::git::error::GitError;
use crate::git::types::{RepoStatus, StashEntry, StashOptions, SwitchOptions};

/// Trait for git operations (enables mocking in tests).
#[cfg_attr(test, mockall::automock)]
#[allow(dead_code)]
pub trait GitOperations: Send + Sync {
    /// Gets the current branch name.
    ///
    /// # Errors
    ///
    /// Returns an error if not in a git repository or HEAD is invalid.
    fn current_branch(&self) -> Result<String>;

    /// Gets the repository status.
    ///
    /// # Errors
    ///
    /// Returns an error if not in a git repository or status cannot be determined.
    fn status(&self) -> Result<RepoStatus>;

    /// Stashes changes with the given options.
    ///
    /// Returns the stash OID as a string.
    ///
    /// # Errors
    ///
    /// Returns an error if stash operation fails.
    fn stash(&mut self, options: StashOptions) -> Result<String>;

    /// Pops the most recent stash.
    ///
    /// # Errors
    ///
    /// Returns an error if stash pop fails or there are conflicts.
    fn stash_pop(&mut self) -> Result<()>;

    /// Applies a stash by name/message without removing it.
    ///
    /// # Errors
    ///
    /// Returns an error if the stash is not found or apply fails.
    fn stash_apply(&self, name: &str) -> Result<()>;

    /// Lists all stashes.
    ///
    /// # Errors
    ///
    /// Returns an error if stash list cannot be retrieved.
    fn stash_list(&self) -> Result<Vec<StashEntry>>;

    /// Drops a stash by index.
    ///
    /// # Errors
    ///
    /// Returns an error if the stash doesn't exist.
    fn stash_drop(&mut self, index: usize) -> Result<()>;

    /// Switches to a branch.
    ///
    /// # Errors
    ///
    /// Returns an error if the branch doesn't exist (and create is false)
    /// or if there are conflicts.
    fn switch_branch(&mut self, branch: &str, options: SwitchOptions) -> Result<()>;

    /// Gets the current commit SHA.
    ///
    /// # Errors
    ///
    /// Returns an error if not in a git repository or HEAD is invalid.
    fn current_commit(&self) -> Result<String>;
}

/// Git operations implementation using git2 library.
#[allow(dead_code)]
pub struct Git2Operations {
    repo_path: std::path::PathBuf,
}

#[allow(dead_code)]
impl Git2Operations {
    /// Opens a repository at the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is not a git repository.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        // Verify it's a valid git repo
        Repository::open(path.as_ref()).map_err(|_| GitError::NotARepository)?;

        Ok(Self {
            repo_path: path.as_ref().to_path_buf(),
        })
    }

    /// Opens a repository from the current directory.
    ///
    /// # Errors
    ///
    /// Returns an error if not in a git repository.
    pub fn from_current_dir() -> Result<Self> {
        let path = std::env::current_dir()
            .map_err(|e| GitError::Git2(format!("Cannot get current directory: {e}")))?;

        // Find repo root by walking up
        let repo = Repository::discover(&path).map_err(|_| GitError::NotARepository)?;

        let workdir = repo
            .workdir()
            .ok_or_else(|| GitError::Git2("Bare repository not supported".to_string()))?;

        Ok(Self {
            repo_path: workdir.to_path_buf(),
        })
    }

    /// Opens the repository (internal helper).
    fn repo(&self) -> Result<Repository> {
        Repository::open(&self.repo_path).map_err(|e| GitError::Git2(e.to_string()).into())
    }

    /// Creates a default signature for stash operations.
    fn default_signature(&self) -> Result<Signature<'static>> {
        let repo = self.repo()?;
        repo.signature()
            .or_else(|_| Signature::now("desk-cli", "desk@localhost"))
            .map_err(|e| GitError::Git2(format!("Cannot create signature: {e}")).into())
    }
}

impl GitOperations for Git2Operations {
    fn current_branch(&self) -> Result<String> {
        let repo = self.repo()?;
        let head = repo
            .head()
            .map_err(|e| GitError::Git2(format!("Cannot get HEAD: {e}")))?;

        if head.is_branch() {
            head.shorthand()
                .map(String::from)
                .ok_or_else(|| GitError::Git2("Invalid branch name".to_string()).into())
        } else {
            // Detached HEAD - return commit SHA
            let commit = head
                .peel_to_commit()
                .map_err(|e| GitError::Git2(format!("Cannot get commit: {e}")))?;
            Ok(format!(
                "HEAD detached at {}",
                &commit.id().to_string()[..7]
            ))
        }
    }

    fn status(&self) -> Result<RepoStatus> {
        let repo = self.repo()?;

        let branch = self.current_branch()?;
        let commit_sha = self.current_commit()?;

        let mut opts = StatusOptions::new();
        opts.include_untracked(true).recurse_untracked_dirs(true);

        let statuses = repo
            .statuses(Some(&mut opts))
            .map_err(|e| GitError::StatusFailed(e.to_string()))?;

        let mut staged = 0;
        let mut modified = 0;
        let mut untracked = 0;

        for entry in statuses.iter() {
            let status = entry.status();

            if status.is_index_new()
                || status.is_index_modified()
                || status.is_index_deleted()
                || status.is_index_renamed()
            {
                staged += 1;
            }
            if status.is_wt_modified() || status.is_wt_deleted() || status.is_wt_renamed() {
                modified += 1;
            }
            if status.is_wt_new() {
                untracked += 1;
            }
        }

        Ok(RepoStatus {
            branch,
            commit_sha,
            is_dirty: staged > 0 || modified > 0,
            staged_count: staged,
            modified_count: modified,
            untracked_count: untracked,
        })
    }

    fn stash(&mut self, options: StashOptions) -> Result<String> {
        let mut repo = self.repo()?;
        let signature = self.default_signature()?;

        let mut flags = StashFlags::DEFAULT;
        if options.include_untracked {
            flags |= StashFlags::INCLUDE_UNTRACKED;
        }

        let message = options.message.as_deref();

        let stash_oid = repo
            .stash_save(
                &signature,
                message.unwrap_or("desk workspace stash"),
                Some(flags),
            )
            .map_err(|e| {
                if e.message().contains("nothing to stash") {
                    GitError::DirtyWorkingDirectory
                } else {
                    GitError::Git2(format!("Stash failed: {e}"))
                }
            })?;

        Ok(stash_oid.to_string())
    }

    fn stash_pop(&mut self) -> Result<()> {
        let mut repo = self.repo()?;

        repo.stash_pop(0, None).map_err(|e| {
            if e.message().contains("conflict") {
                GitError::Conflict(e.message().to_string())
            } else {
                GitError::Git2(format!("Stash pop failed: {e}"))
            }
        })?;

        Ok(())
    }

    fn stash_apply(&self, name: &str) -> Result<()> {
        let mut repo = self.repo()?;

        // Find stash by message
        let mut found_index = None;
        repo.stash_foreach(|index, message, _oid| {
            if message.contains(name) {
                found_index = Some(index);
                false // Stop iteration
            } else {
                true // Continue
            }
        })
        .map_err(|e| GitError::Git2(e.to_string()))?;

        let index = found_index.ok_or_else(|| GitError::StashNotFound(name.to_string()))?;

        repo.stash_apply(index, None)
            .map_err(|e| GitError::Git2(format!("Stash apply failed: {e}")))?;

        Ok(())
    }

    fn stash_list(&self) -> Result<Vec<StashEntry>> {
        let mut repo = self.repo()?;
        let mut entries = Vec::new();

        repo.stash_foreach(|index, message, _oid| {
            entries.push(StashEntry {
                index,
                message: message.to_string(),
                branch: None, // Could parse from message if needed
            });
            true // Continue iteration
        })
        .map_err(|e| GitError::Git2(e.to_string()))?;

        Ok(entries)
    }

    fn stash_drop(&mut self, index: usize) -> Result<()> {
        let mut repo = self.repo()?;

        repo.stash_drop(index)
            .map_err(|e| GitError::Git2(format!("Stash drop failed: {e}")))?;

        Ok(())
    }

    fn switch_branch(&mut self, branch: &str, options: SwitchOptions) -> Result<()> {
        let repo = self.repo()?;

        // Check if branch exists
        let reference = if options.create {
            // Create new branch from HEAD
            let head = repo
                .head()
                .map_err(|e| GitError::Git2(format!("Cannot get HEAD: {e}")))?;
            let commit = head
                .peel_to_commit()
                .map_err(|e| GitError::Git2(format!("Cannot get commit: {e}")))?;

            repo.branch(branch, &commit, false)
                .map_err(|e| GitError::Git2(format!("Cannot create branch: {e}")))?
                .into_reference()
        } else {
            repo.find_branch(branch, git2::BranchType::Local)
                .map_err(|_| GitError::BranchNotFound(branch.to_string()))?
                .into_reference()
        };

        // Checkout the branch
        let tree = reference
            .peel_to_tree()
            .map_err(|e| GitError::Git2(format!("Cannot get tree: {e}")))?;

        let mut checkout_opts = git2::build::CheckoutBuilder::new();
        if options.force {
            checkout_opts.force();
        } else {
            checkout_opts.safe();
        }

        repo.checkout_tree(tree.as_object(), Some(&mut checkout_opts))
            .map_err(|e| {
                if e.message().contains("conflict") {
                    GitError::Conflict(e.message().to_string())
                } else {
                    GitError::Git2(format!("Checkout failed: {e}"))
                }
            })?;

        // Update HEAD
        let ref_name = reference
            .name()
            .ok_or_else(|| GitError::Git2("Invalid reference name".to_string()))?;
        repo.set_head(ref_name)
            .map_err(|e| GitError::Git2(format!("Cannot update HEAD: {e}")))?;

        Ok(())
    }

    fn current_commit(&self) -> Result<String> {
        let repo = self.repo()?;
        let head = repo
            .head()
            .map_err(|e| GitError::Git2(format!("Cannot get HEAD: {e}")))?;
        let commit = head
            .peel_to_commit()
            .map_err(|e| GitError::Git2(format!("Cannot get commit: {e}")))?;

        Ok(commit.id().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn init_test_repo() -> (TempDir, Repository) {
        let temp_dir = TempDir::new().unwrap();
        let repo = Repository::init(temp_dir.path()).unwrap();

        // Create initial commit
        {
            let sig = Signature::now("Test", "test@test.com").unwrap();
            let tree_id = repo.index().unwrap().write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
                .unwrap();
        }

        (temp_dir, repo)
    }

    #[test]
    fn open_valid_repository() {
        let (temp_dir, _repo) = init_test_repo();
        let ops = Git2Operations::open(temp_dir.path());
        assert!(ops.is_ok());
    }

    #[test]
    fn open_invalid_repository() {
        let temp_dir = TempDir::new().unwrap();
        let ops = Git2Operations::open(temp_dir.path());
        assert!(ops.is_err());
    }

    #[test]
    fn current_branch_on_main() {
        let (temp_dir, _repo) = init_test_repo();
        let ops = Git2Operations::open(temp_dir.path()).unwrap();

        // Default branch might be "master" or "main" depending on git config
        let branch = ops.current_branch().unwrap();
        assert!(!branch.is_empty());
    }

    #[test]
    fn current_commit_returns_sha() {
        let (temp_dir, _repo) = init_test_repo();
        let ops = Git2Operations::open(temp_dir.path()).unwrap();

        let sha = ops.current_commit().unwrap();
        assert_eq!(sha.len(), 40); // Full SHA
    }

    #[test]
    fn status_clean_repo() {
        let (temp_dir, _repo) = init_test_repo();
        let ops = Git2Operations::open(temp_dir.path()).unwrap();

        let status = ops.status().unwrap();
        assert!(!status.is_dirty);
        assert_eq!(status.staged_count, 0);
        assert_eq!(status.modified_count, 0);
    }

    #[test]
    fn status_with_untracked_file() {
        let (temp_dir, _repo) = init_test_repo();
        let ops = Git2Operations::open(temp_dir.path()).unwrap();

        // Create an untracked file
        fs::write(temp_dir.path().join("untracked.txt"), "content").unwrap();

        let status = ops.status().unwrap();
        assert_eq!(status.untracked_count, 1);
    }

    #[test]
    fn switch_to_new_branch() {
        let (temp_dir, _repo) = init_test_repo();
        let mut ops = Git2Operations::open(temp_dir.path()).unwrap();

        let options = SwitchOptions {
            create: true,
            force: false,
        };
        ops.switch_branch("feature/test", options).unwrap();

        let branch = ops.current_branch().unwrap();
        assert_eq!(branch, "feature/test");
    }

    #[test]
    fn switch_to_nonexistent_branch_fails() {
        let (temp_dir, _repo) = init_test_repo();
        let mut ops = Git2Operations::open(temp_dir.path()).unwrap();

        let result = ops.switch_branch("nonexistent", SwitchOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn stash_list_empty() {
        let (temp_dir, _repo) = init_test_repo();
        let ops = Git2Operations::open(temp_dir.path()).unwrap();

        let stashes = ops.stash_list().unwrap();
        assert!(stashes.is_empty());
    }
}
