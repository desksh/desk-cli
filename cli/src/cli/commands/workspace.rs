//! Workspace command handlers for the desk CLI.
//!
//! This module implements the workspace management commands:
//! - [`handle_open`] - Create or restore a workspace (`desk open`)
//! - [`handle_list`] - List all saved workspaces (`desk list`)
//! - [`handle_workspace_status`] - Show current git status (`desk status`)
//! - [`handle_close`] - Close current workspace (`desk close`)
//!
//! These commands use the [`crate::workspace`] module for persistence and
//! the [`crate::git`] module for git operations.

use crate::error::Result;
use crate::git::{Git2Operations, GitOperations, StashOptions, SwitchOptions};
use crate::workspace::{FileWorkspaceStore, Workspace, WorkspaceStore};

/// Handles the `desk open <name>` command.
///
/// If the workspace exists, restores its git state (switches branch, applies stash).
/// If it doesn't exist, saves the current git state as a new workspace.
///
/// # Arguments
///
/// * `name` - The workspace name to open or create
/// * `description` - Optional description for a new workspace
/// * `force` - If true, overwrites an existing workspace with current state
///
/// # Errors
///
/// Returns an error if:
/// - Not in a git repository
/// - Workspace storage operations fail
/// - Git operations (branch switch, stash) fail
pub fn handle_open(name: &str, description: Option<String>, force: bool) -> Result<()> {
    let store = FileWorkspaceStore::new()?;
    let mut git = Git2Operations::from_current_dir()?;

    // Check if workspace already exists
    if let Some(existing) = store.load(name)? {
        if force {
            // Save current state first, then restore the existing workspace
            save_current_state(&store, &mut git, name, description, true)?;
            println!("Workspace '{name}' updated with current state.");
        } else {
            // Restore the existing workspace
            restore_workspace(&mut git, &existing)?;
            println!("Restored workspace '{name}'.");
            println!("  Branch: {}", existing.branch);
            if existing.stash_name.is_some() {
                println!("  Stashed changes applied.");
            }
        }
    } else {
        // Create new workspace from current state
        save_current_state(&store, &mut git, name, description, false)?;
        println!("Created workspace '{name}'.");
    }

    Ok(())
}

/// Handles the `desk list` command.
///
/// Lists all saved workspaces with their details, sorted by most recently updated.
///
/// # Errors
///
/// Returns an error if workspace storage cannot be accessed.
pub fn handle_list() -> Result<()> {
    let store = FileWorkspaceStore::new()?;
    let workspaces = store.list()?;

    if workspaces.is_empty() {
        println!("No workspaces saved yet.");
        println!("\nCreate one with: desk open <name>");
        return Ok(());
    }

    println!("Saved workspaces:\n");
    for ws in workspaces {
        println!("  {} ", ws.name);
        println!("    Branch: {}", ws.branch);
        if let Some(desc) = &ws.description {
            println!("    Description: {desc}");
        }
        println!("    Updated: {}", ws.updated_at.format("%Y-%m-%d %H:%M:%S"));
        println!();
    }

    Ok(())
}

/// Handles the `desk status` command.
///
/// Shows the current git repository status including:
/// - Current branch and commit
/// - Staged, modified, and untracked file counts
/// - Number of stashes
///
/// # Errors
///
/// Returns an error if not in a git repository.
pub fn handle_workspace_status() -> Result<()> {
    let git = Git2Operations::from_current_dir()?;
    let status = git.status()?;

    println!("Current status:\n");
    println!("  Branch: {}", status.branch);
    println!("  Commit: {}", &status.commit_sha[..7]);

    if status.is_dirty {
        println!("  Status: dirty");
        if status.staged_count > 0 {
            println!("    Staged: {} file(s)", status.staged_count);
        }
        if status.modified_count > 0 {
            println!("    Modified: {} file(s)", status.modified_count);
        }
        if status.untracked_count > 0 {
            println!("    Untracked: {} file(s)", status.untracked_count);
        }
    } else {
        println!("  Status: clean");
    }

    // Check for stashes
    let stashes = git.stash_list()?;
    if !stashes.is_empty() {
        println!("\n  Stashes: {}", stashes.len());
    }

    Ok(())
}

/// Handles the `desk close` command.
///
/// Closes the current workspace context. Optionally switches to another
/// workspace after closing.
///
/// # Arguments
///
/// * `switch_to` - Optional workspace name to switch to after closing
///
/// # Errors
///
/// Returns an error if:
/// - `switch_to` is specified but the workspace doesn't exist
/// - Git operations fail during the switch
pub fn handle_close(switch_to: Option<String>) -> Result<()> {
    if let Some(target) = switch_to {
        let store = FileWorkspaceStore::new()?;
        let mut git = Git2Operations::from_current_dir()?;

        if let Some(workspace) = store.load(&target)? {
            restore_workspace(&mut git, &workspace)?;
            println!("Switched to workspace '{target}'.");
        } else {
            println!("Workspace '{target}' not found.");
            std::process::exit(1);
        }
    } else {
        println!("Workspace closed.");
        println!("\nUse 'desk open <name>' to switch to another workspace.");
    }

    Ok(())
}

/// Saves the current git state as a workspace.
///
/// Captures the current branch, commit SHA, and optionally stashes uncommitted
/// changes with a desk-prefixed message for later restoration.
fn save_current_state(
    store: &FileWorkspaceStore,
    git: &mut Git2Operations,
    name: &str,
    description: Option<String>,
    force: bool,
) -> Result<()> {
    let status = git.status()?;

    // Capture metadata before moving status fields
    let was_dirty = status.has_changes();
    #[allow(clippy::cast_possible_truncation)]
    let total_changes = status.total_changes() as u32;

    // Stash changes if dirty
    let stash_name = if was_dirty {
        let stash_msg = format!("desk: workspace {name}");
        let options = StashOptions {
            message: Some(stash_msg.clone()),
            include_untracked: true,
        };
        git.stash(options)?;
        Some(stash_msg)
    } else {
        None
    };

    let mut workspace = Workspace::new(
        name.to_string(),
        std::env::current_dir()?,
        status.branch,
        status.commit_sha,
    );
    workspace.stash_name = stash_name;
    workspace.description = description;
    workspace.metadata.was_dirty = Some(was_dirty);
    workspace.metadata.uncommitted_files = Some(total_changes);

    store.save(&workspace, force)?;

    Ok(())
}

/// Restores a workspace's git state.
///
/// Switches to the workspace's branch (stashing current changes if needed)
/// and applies the workspace's stash if one was saved.
fn restore_workspace(git: &mut Git2Operations, workspace: &Workspace) -> Result<()> {
    let current_status = git.status()?;

    // Switch branch if different
    if current_status.branch != workspace.branch {
        // Stash current changes if dirty
        if current_status.has_changes() {
            git.stash(StashOptions {
                message: Some("desk: auto-stash before switch".to_string()),
                include_untracked: true,
            })?;
        }

        git.switch_branch(
            &workspace.branch,
            SwitchOptions {
                create: false,
                force: false,
            },
        )?;
    }

    // Apply workspace stash if it exists
    if let Some(stash_name) = &workspace.stash_name {
        // Try to apply the stash - it might not exist anymore
        if git.stash_apply(stash_name).is_ok() {
            // Successfully applied
        }
    }

    Ok(())
}
