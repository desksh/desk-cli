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

use std::io::{self, Write};

use crate::cli::args::ShellType;
use crate::client::DeskApiClient;
use crate::config::{load_config, DeskState};
use crate::error::{DeskError, Result};
use crate::git::{Git2Operations, GitOperations, StashOptions, SwitchOptions};
use crate::workspace::{FileWorkspaceStore, Workspace, WorkspaceStore};

/// Handles the `desk open <name>` command.
///
/// If the workspace exists, restores its git state (switches branch, applies stash).
/// If it doesn't exist, saves the current git state as a new workspace.
///
/// # Arguments
///
/// * `name` - The workspace name to open or create (optional if interactive)
/// * `description` - Optional description for a new workspace
/// * `force` - If true, overwrites an existing workspace with current state
/// * `interactive` - If true, show interactive selection menu
///
/// # Errors
///
/// Returns an error if:
/// - Not in a git repository
/// - Workspace storage operations fail
/// - Git operations (branch switch, stash) fail
pub fn handle_open(
    name: Option<String>,
    description: Option<String>,
    force: bool,
    interactive: bool,
) -> Result<()> {
    // Handle interactive mode
    if interactive {
        return handle_interactive_open();
    }

    // Name is required for non-interactive mode
    let Some(name) = name else {
        println!("Workspace name required. Use --interactive to select from a list.");
        std::process::exit(1);
    };

    // Resolve alias if applicable
    let state = DeskState::load()?;
    let resolved_name = state.resolve_alias(&name);

    let store = FileWorkspaceStore::new()?;
    let mut git = Git2Operations::from_current_dir()?;
    let repo_path = std::env::current_dir()?;

    // Check if workspace already exists
    if let Some(mut existing) = store.load(&resolved_name)? {
        if force {
            // Save current state first, then restore the existing workspace
            save_current_state(&store, &mut git, &resolved_name, description, true)?;
            println!("Workspace '{resolved_name}' updated with current state.");
        } else {
            // Restore the existing workspace
            restore_workspace(&mut git, &existing)?;
            println!("Restored workspace '{resolved_name}'.");
            println!("  Branch: {}", existing.branch);
            if existing.stash_name.is_some() {
                println!("  Stashed changes applied.");
            }

            // Update usage stats
            existing.metadata.open_count += 1;
            existing.metadata.last_opened_at = Some(chrono::Utc::now());
            store.save(&existing, true)?;
        }
    } else {
        // Create new workspace from current state
        save_current_state(&store, &mut git, &resolved_name, description, false)?;
        println!("Created workspace '{resolved_name}'.");
    }

    // Track current workspace in state
    let mut state = DeskState::load()?;
    state.set_current(&repo_path, &resolved_name);
    state.record_workspace_opened(&repo_path);
    state.save()?;

    Ok(())
}

/// Handles the `desk list` command.
///
/// Lists all saved workspaces with their details, sorted by most recently updated.
///
/// # Arguments
///
/// * `tag` - Optional tag to filter by
/// * `archived` - If true, show only archived workspaces
/// * `all` - If true, show all workspaces including archived
///
/// # Errors
///
/// Returns an error if workspace storage cannot be accessed.
pub fn handle_list(tag: Option<String>, archived: bool, all: bool) -> Result<()> {
    let store = FileWorkspaceStore::new()?;
    let workspaces = store.list()?;

    if workspaces.is_empty() {
        println!("No workspaces saved yet.");
        println!("\nCreate one with: desk open <name>");
        return Ok(());
    }

    // Filter workspaces
    let filtered: Vec<_> = workspaces
        .iter()
        .filter(|ws| {
            // Archive filter
            let archive_match = if archived {
                ws.metadata.archived
            } else if all {
                true
            } else {
                !ws.metadata.archived
            };

            // Tag filter
            let tag_match = tag.as_ref().map_or(true, |t| ws.metadata.tags.contains(t));

            archive_match && tag_match
        })
        .collect();

    if filtered.is_empty() {
        if tag.is_some() {
            println!("No workspaces found with tag '{}'.", tag.unwrap());
        } else if archived {
            println!("No archived workspaces.");
        } else {
            println!("No active workspaces.");
        }
        return Ok(());
    }

    let header = if archived {
        "Archived workspaces"
    } else if all {
        "All workspaces"
    } else {
        "Saved workspaces"
    };

    println!("{header}:\n");
    for ws in filtered {
        let archive_marker = if ws.metadata.archived {
            " [archived]"
        } else {
            ""
        };
        let tags_str = if ws.metadata.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", ws.metadata.tags.join(", "))
        };

        println!("  {}{}{}", ws.name, archive_marker, tags_str);
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
    let repo_path = std::env::current_dir()?;
    let mut state = DeskState::load()?;

    if let Some(target) = switch_to {
        let store = FileWorkspaceStore::new()?;
        let mut git = Git2Operations::from_current_dir()?;

        if let Some(workspace) = store.load(&target)? {
            restore_workspace(&mut git, &workspace)?;
            // Update state to new workspace
            state.set_current(&repo_path, &target);
            state.save()?;
            println!("Switched to workspace '{target}'.");
        } else {
            println!("Workspace '{target}' not found.");
            std::process::exit(1);
        }
    } else {
        // Clear current workspace
        state.clear_current(&repo_path);
        state.save()?;
        println!("Workspace closed.");
        println!("\nUse 'desk open <name>' to switch to another workspace.");
    }

    Ok(())
}

/// Handles the `desk delete <name>` command.
///
/// Deletes a workspace locally and optionally from the cloud.
///
/// # Arguments
///
/// * `name` - The workspace name to delete
/// * `cloud` - If true, also delete from the cloud (if synced)
/// * `yes` - If true, skip confirmation prompt
///
/// # Errors
///
/// Returns an error if:
/// - Workspace doesn't exist
/// - Cloud deletion fails (when `--cloud` is specified)
/// - User cancels the operation
pub async fn handle_delete(name: &str, cloud: bool, yes: bool) -> Result<()> {
    let store = FileWorkspaceStore::new()?;

    // Load the workspace to check if it exists and get remote_id
    let Some(workspace) = store.load(name)? else {
        println!("Workspace '{name}' not found.");
        std::process::exit(1);
    };

    // Check if workspace is synced to cloud
    let is_synced = workspace.metadata.remote_id.is_some();
    let remote_id = workspace.metadata.remote_id.clone();

    // Confirm deletion unless --yes is specified
    if !yes {
        let cloud_msg = if cloud && is_synced {
            " (including cloud copy)"
        } else if is_synced && !cloud {
            " (cloud copy will be preserved)"
        } else {
            ""
        };

        print!("Delete workspace '{name}'{cloud_msg}? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Delete from cloud if requested and synced
    if cloud && is_synced {
        if let Some(ref id) = remote_id {
            let config = load_config()?;
            let client = DeskApiClient::new(&config.api)?;

            // Load credentials
            if !client.load_credentials().await? {
                return Err(DeskError::NotAuthenticated);
            }

            match client.delete_workspace(id).await {
                Ok(()) => {
                    println!("Deleted from cloud.");
                },
                Err(DeskError::SubscriptionRequired) => {
                    println!("Warning: Could not delete from cloud (Pro subscription required).");
                },
                Err(e) => {
                    println!("Warning: Failed to delete from cloud: {e}");
                },
            }
        }
    }

    // Delete locally
    if store.delete(name)? {
        println!("Deleted workspace '{name}'.");
    } else {
        println!("Workspace '{name}' was already deleted.");
    }

    Ok(())
}

/// Handles the `desk rename <name> <new_name>` command.
///
/// Renames a workspace locally and optionally on the cloud.
///
/// # Arguments
///
/// * `name` - The current workspace name
/// * `new_name` - The new name for the workspace
/// * `cloud` - If true, also rename on the cloud (if synced)
///
/// # Errors
///
/// Returns an error if:
/// - Workspace doesn't exist
/// - New name already exists
/// - Cloud rename fails (when `--cloud` is specified)
pub async fn handle_rename(name: &str, new_name: &str, cloud: bool) -> Result<()> {
    let store = FileWorkspaceStore::new()?;

    // Check if source workspace exists
    let Some(mut workspace) = store.load(name)? else {
        println!("Workspace '{name}' not found.");
        std::process::exit(1);
    };

    // Check if target name already exists
    if store.exists(new_name)? {
        println!("Workspace '{new_name}' already exists.");
        std::process::exit(1);
    }

    // Check if workspace is synced to cloud
    let is_synced = workspace.metadata.remote_id.is_some();
    let remote_id = workspace.metadata.remote_id.clone();

    // Rename on cloud if requested and synced
    if cloud && is_synced {
        if let Some(ref id) = remote_id {
            let config = load_config()?;
            let client = DeskApiClient::new(&config.api)?;

            // Load credentials
            if !client.load_credentials().await? {
                return Err(DeskError::NotAuthenticated);
            }

            let version = workspace.metadata.remote_version.unwrap_or(0);

            match client
                .update_workspace(id, Some(new_name), None, None, version)
                .await
            {
                Ok(updated) => {
                    // Update version from server response
                    workspace.metadata.remote_version = Some(updated.version);
                    println!("Renamed on cloud.");
                },
                Err(DeskError::SubscriptionRequired) => {
                    println!("Warning: Could not rename on cloud (Pro subscription required).");
                },
                Err(e) => {
                    println!("Warning: Failed to rename on cloud: {e}");
                    println!("Local rename will still proceed.");
                },
            }
        }
    } else if cloud && !is_synced {
        println!("Note: Workspace is not synced to cloud, skipping cloud rename.");
    }

    // Update the workspace name
    workspace.name = new_name.to_string();
    workspace.touch();

    // Save with new name
    store.save(&workspace, false)?;

    // Delete old workspace file
    store.delete(name)?;

    println!("Renamed workspace '{name}' to '{new_name}'.");

    Ok(())
}

/// Handles the `desk info <name>` command.
///
/// Shows detailed information about a workspace.
///
/// # Arguments
///
/// * `name` - The workspace name to inspect
///
/// # Errors
///
/// Returns an error if the workspace doesn't exist.
pub fn handle_info(name: &str) -> Result<()> {
    let store = FileWorkspaceStore::new()?;

    let Some(workspace) = store.load(name)? else {
        println!("Workspace '{name}' not found.");
        std::process::exit(1);
    };

    println!("Workspace: {}\n", workspace.name);

    // Description
    if let Some(ref desc) = workspace.description {
        println!("  Description: {desc}");
    }

    // Git state
    println!("  Repository:  {}", workspace.repo_path.display());
    println!("  Branch:      {}", workspace.branch);
    println!("  Commit:      {}", workspace.commit_sha);

    if let Some(ref stash) = workspace.stash_name {
        println!("  Stash:       {stash}");
    }

    // Timestamps
    println!();
    println!(
        "  Created:     {}",
        workspace.created_at.format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!(
        "  Updated:     {}",
        workspace.updated_at.format("%Y-%m-%d %H:%M:%S UTC")
    );

    // Metadata
    if workspace.metadata.was_dirty.is_some() || workspace.metadata.uncommitted_files.is_some() {
        println!();
        println!("  State at save:");
        if let Some(dirty) = workspace.metadata.was_dirty {
            println!(
                "    Working directory: {}",
                if dirty { "dirty" } else { "clean" }
            );
        }
        if let Some(count) = workspace.metadata.uncommitted_files {
            println!("    Uncommitted files: {count}");
        }
    }

    // Sync status
    println!();
    if workspace.metadata.remote_id.is_some() {
        println!("  Sync status: synced");
        if let Some(ref id) = workspace.metadata.remote_id {
            println!("    Remote ID: {id}");
        }
        if let Some(version) = workspace.metadata.remote_version {
            println!("    Version:   {version}");
        }
        if let Some(synced_at) = workspace.metadata.last_synced_at {
            println!(
                "    Last sync: {}",
                synced_at.format("%Y-%m-%d %H:%M:%S UTC")
            );
        }
    } else {
        println!("  Sync status: local only");
    }

    Ok(())
}

/// Handles the `desk clone <name> <new_name>` command.
///
/// Creates a copy of an existing workspace with a new name.
///
/// # Arguments
///
/// * `name` - The workspace to clone
/// * `new_name` - The name for the new workspace
///
/// # Errors
///
/// Returns an error if the source workspace doesn't exist or target already exists.
pub fn handle_clone(name: &str, new_name: &str) -> Result<()> {
    let store = FileWorkspaceStore::new()?;

    // Load source workspace
    let Some(workspace) = store.load(name)? else {
        println!("Workspace '{name}' not found.");
        std::process::exit(1);
    };

    // Check if target already exists
    if store.exists(new_name)? {
        println!("Workspace '{new_name}' already exists.");
        std::process::exit(1);
    }

    // Create clone with new name
    let mut cloned = workspace;
    cloned.name = new_name.to_string();
    cloned.touch();
    // Clear sync metadata - clone is a new local workspace
    cloned.metadata.remote_id = None;
    cloned.metadata.remote_version = None;
    cloned.metadata.last_synced_at = None;

    store.save(&cloned, false)?;

    println!("Cloned workspace '{name}' to '{new_name}'.");

    Ok(())
}

/// Handles the `desk describe <name> <description>` command.
///
/// Updates the description of an existing workspace.
///
/// # Arguments
///
/// * `name` - The workspace name
/// * `description` - The new description
/// * `cloud` - If true, also update on the cloud
///
/// # Errors
///
/// Returns an error if the workspace doesn't exist.
pub async fn handle_describe(name: &str, description: &str, cloud: bool) -> Result<()> {
    let store = FileWorkspaceStore::new()?;

    // Load workspace
    let Some(mut workspace) = store.load(name)? else {
        println!("Workspace '{name}' not found.");
        std::process::exit(1);
    };

    let is_synced = workspace.metadata.remote_id.is_some();
    let remote_id = workspace.metadata.remote_id.clone();

    // Update on cloud if requested and synced
    if cloud && is_synced {
        if let Some(ref id) = remote_id {
            let config = load_config()?;
            let client = DeskApiClient::new(&config.api)?;

            if !client.load_credentials().await? {
                return Err(DeskError::NotAuthenticated);
            }

            let version = workspace.metadata.remote_version.unwrap_or(0);

            match client
                .update_workspace(id, None, Some(description), None, version)
                .await
            {
                Ok(updated) => {
                    workspace.metadata.remote_version = Some(updated.version);
                    println!("Updated description on cloud.");
                },
                Err(DeskError::SubscriptionRequired) => {
                    println!("Warning: Could not update cloud (Pro subscription required).");
                },
                Err(e) => {
                    println!("Warning: Failed to update cloud: {e}");
                },
            }
        }
    } else if cloud && !is_synced {
        println!("Note: Workspace is not synced to cloud.");
    }

    // Update locally
    workspace.description = Some(description.to_string());
    workspace.touch();
    store.save(&workspace, true)?;

    println!("Updated description for workspace '{name}'.");

    Ok(())
}

/// Handles the `desk export <name>` command.
///
/// Exports a workspace to a JSON file.
///
/// # Arguments
///
/// * `name` - The workspace to export
/// * `output` - Optional output file path
///
/// # Errors
///
/// Returns an error if the workspace doesn't exist or file write fails.
pub fn handle_export(name: &str, output: Option<String>) -> Result<()> {
    let store = FileWorkspaceStore::new()?;

    let Some(workspace) = store.load(name)? else {
        println!("Workspace '{name}' not found.");
        std::process::exit(1);
    };

    let output_path = output.unwrap_or_else(|| format!("{name}.json"));

    let json = serde_json::to_string_pretty(&workspace)?;
    std::fs::write(&output_path, json)?;

    println!("Exported workspace '{name}' to '{output_path}'.");

    Ok(())
}

/// Handles the `desk import <file>` command.
///
/// Imports a workspace from a JSON file.
///
/// # Arguments
///
/// * `file` - Path to the JSON file
/// * `name` - Optional override for the workspace name
/// * `force` - If true, overwrite existing workspace
///
/// # Errors
///
/// Returns an error if the file doesn't exist or is invalid.
pub fn handle_import(file: &str, name: Option<String>, force: bool) -> Result<()> {
    let store = FileWorkspaceStore::new()?;

    // Read and parse the file
    let contents = std::fs::read_to_string(file).map_err(|e| {
        println!("Failed to read file '{file}': {e}");
        e
    })?;

    let mut workspace: Workspace = serde_json::from_str(&contents).map_err(|e| {
        println!("Invalid workspace file: {e}");
        e
    })?;

    // Override name if provided
    if let Some(new_name) = name {
        workspace.name = new_name;
    }

    // Check if already exists
    if !force && store.exists(&workspace.name)? {
        println!(
            "Workspace '{}' already exists. Use --force to overwrite.",
            workspace.name
        );
        std::process::exit(1);
    }

    // Clear sync metadata on import
    workspace.metadata.remote_id = None;
    workspace.metadata.remote_version = None;
    workspace.metadata.last_synced_at = None;
    workspace.touch();

    store.save(&workspace, force)?;

    println!("Imported workspace '{}'.", workspace.name);

    Ok(())
}

/// Handles the `desk clean` command.
///
/// Cleans up orphaned desk stashes from git.
///
/// # Arguments
///
/// * `execute` - If true, actually delete stashes. Otherwise, dry-run.
///
/// # Errors
///
/// Returns an error if git operations fail.
pub fn handle_clean(execute: bool) -> Result<()> {
    let store = FileWorkspaceStore::new()?;
    let mut git = Git2Operations::from_current_dir()?;

    // Get all stashes
    let stashes = git.stash_list()?;

    // Get all workspace stash names
    let workspaces = store.list()?;
    let referenced_stashes: std::collections::HashSet<_> = workspaces
        .iter()
        .filter_map(|ws| ws.stash_name.as_ref())
        .collect();

    // Find orphaned desk stashes
    let orphaned: Vec<_> = stashes
        .iter()
        .filter(|s| s.message.starts_with("desk:") && !referenced_stashes.contains(&s.message))
        .collect();

    if orphaned.is_empty() {
        println!("No orphaned desk stashes found.");
        return Ok(());
    }

    println!("Found {} orphaned desk stash(es):\n", orphaned.len());

    for stash in &orphaned {
        println!("  stash@{{{}}}: {}", stash.index, stash.message);
    }

    println!();
    if execute {
        // Drop stashes in reverse order to preserve indices
        let mut dropped = 0;
        for stash in orphaned.iter().rev() {
            if git.stash_drop(stash.index).is_ok() {
                dropped += 1;
            }
        }
        println!("Dropped {dropped} stash(es).");
    } else {
        println!("Run with --execute to delete these stashes.");
    }

    Ok(())
}

/// Handles the `desk prompt` command.
///
/// Outputs the current workspace name for use in shell prompts.
/// If no workspace is active, outputs nothing.
pub fn handle_prompt() {
    let Ok(repo_path) = std::env::current_dir() else {
        return; // Silently fail for prompt
    };

    let state = DeskState::load().unwrap_or_default();

    if let Some(workspace_name) = state.get_current(&repo_path) {
        print!("{workspace_name}");
    }
}

/// Handles the `desk init <shell>` command.
///
/// Generates shell integration script for the specified shell.
///
/// # Arguments
///
/// * `shell` - The shell type to generate script for
pub fn handle_init(shell: ShellType) {
    let script = match shell {
        ShellType::Bash => BASH_INIT_SCRIPT,
        ShellType::Zsh => ZSH_INIT_SCRIPT,
        ShellType::Fish => FISH_INIT_SCRIPT,
    };

    println!("{script}");
}

const BASH_INIT_SCRIPT: &str = r#"# Desk shell integration for Bash
# Add this to your ~/.bashrc

_desk_prompt() {
    local ws
    ws=$(desk prompt 2>/dev/null)
    if [[ -n "$ws" ]]; then
        echo " [$ws]"
    fi
}

# Add to PS1 - example: PS1='\u@\h:\w$(_desk_prompt)\$ '
# Or use PROMPT_COMMAND for dynamic updates:
# PROMPT_COMMAND='PS1="\u@\h:\w$(_desk_prompt)\$ "'
"#;

const ZSH_INIT_SCRIPT: &str = r#"# Desk shell integration for Zsh
# Add this to your ~/.zshrc

_desk_prompt() {
    local ws
    ws=$(desk prompt 2>/dev/null)
    if [[ -n "$ws" ]]; then
        echo " [$ws]"
    fi
}

# Add to your prompt - example:
# PROMPT='%n@%m:%~$(_desk_prompt)%# '

# Or use precmd hook for dynamic updates:
# precmd() { PROMPT="%n@%m:%~$(_desk_prompt)%# " }
"#;

const FISH_INIT_SCRIPT: &str = r#"# Desk shell integration for Fish
# Add this to your ~/.config/fish/config.fish

function _desk_prompt
    set -l ws (desk prompt 2>/dev/null)
    if test -n "$ws"
        echo " [$ws]"
    end
end

# Add to your fish_prompt function - example:
# function fish_prompt
#     echo -n (whoami)'@'(hostname)':'(pwd)(_desk_prompt)'> '
# end
"#;

/// Handles the `desk search <query>` command.
///
/// Searches workspaces by name, branch, or description.
///
/// # Arguments
///
/// * `query` - Search query string
/// * `name_only` - Only search workspace names
/// * `branch_only` - Only search branch names
pub fn handle_search(query: &str, name_only: bool, branch_only: bool) -> Result<()> {
    let store = FileWorkspaceStore::new()?;
    let workspaces = store.list()?;

    let query_lower = query.to_lowercase();

    let matches: Vec<_> = workspaces
        .iter()
        .filter(|ws| {
            if name_only {
                ws.name.to_lowercase().contains(&query_lower)
            } else if branch_only {
                ws.branch.to_lowercase().contains(&query_lower)
            } else {
                ws.name.to_lowercase().contains(&query_lower)
                    || ws.branch.to_lowercase().contains(&query_lower)
                    || ws
                        .description
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&query_lower))
            }
        })
        .collect();

    if matches.is_empty() {
        println!("No workspaces found matching '{query}'.");
        return Ok(());
    }

    println!("Found {} workspace(s):\n", matches.len());

    for ws in matches {
        println!("  {}", ws.name);
        println!("    Branch: {}", ws.branch);
        if let Some(ref desc) = ws.description {
            println!("    Description: {desc}");
        }
        println!();
    }

    Ok(())
}

/// Handles the `desk completions <shell>` command.
///
/// Generates shell completion scripts.
pub fn handle_completions(shell: ShellType) {
    use clap::CommandFactory;
    use clap_complete::{generate, Shell};

    let mut cmd = crate::cli::Cli::command();
    let shell = match shell {
        ShellType::Bash => Shell::Bash,
        ShellType::Zsh => Shell::Zsh,
        ShellType::Fish => Shell::Fish,
    };

    generate(shell, &mut cmd, "desk", &mut std::io::stdout());
}

/// Handles the `desk doctor` command.
///
/// Checks desk installation and diagnoses issues.
#[allow(clippy::too_many_lines)]
pub fn handle_doctor() -> Result<()> {
    println!("Desk Doctor\n");
    println!("Checking installation...\n");

    let mut issues = 0;

    // Check 1: Config directory
    print!("  Config directory: ");
    match crate::config::paths::config_dir() {
        Ok(path) => {
            if path.exists() {
                println!("OK ({})", path.display());
            } else {
                println!("OK (will be created: {})", path.display());
            }
        },
        Err(e) => {
            println!("ERROR - {e}");
            issues += 1;
        },
    }

    // Check 2: Data directory
    print!("  Data directory: ");
    match crate::config::paths::data_dir() {
        Ok(path) => {
            if path.exists() {
                println!("OK ({})", path.display());
            } else {
                println!("OK (will be created: {})", path.display());
            }
        },
        Err(e) => {
            println!("ERROR - {e}");
            issues += 1;
        },
    }

    // Check 3: Workspaces directory
    print!("  Workspaces directory: ");
    match crate::config::paths::workspaces_dir() {
        Ok(path) => {
            if path.exists() {
                // Count workspaces
                let store = FileWorkspaceStore::new()?;
                let count = store.list()?.len();
                println!("OK ({count} workspace(s))");
            } else {
                println!("OK (will be created)");
            }
        },
        Err(e) => {
            println!("ERROR - {e}");
            issues += 1;
        },
    }

    // Check 4: Git access
    print!("  Git repository: ");
    match Git2Operations::from_current_dir() {
        Ok(git) => match git.status() {
            Ok(status) => {
                println!(
                    "OK (branch: {}, {})",
                    status.branch,
                    if status.is_dirty { "dirty" } else { "clean" }
                );
            },
            Err(e) => {
                println!("WARNING - {e}");
            },
        },
        Err(_) => {
            println!("N/A (not in a git repository)");
        },
    }

    // Check 5: Config file
    print!("  Configuration: ");
    match crate::config::load_config() {
        Ok(config) => {
            println!("OK (API: {})", config.api.base_url);
        },
        Err(e) => {
            println!("ERROR - {e}");
            issues += 1;
        },
    }

    // Check 6: Authentication
    print!("  Authentication: ");
    match crate::auth::credentials::CredentialStore::new() {
        Ok(store) => match store.load() {
            Ok(Some(creds)) => {
                if creds.is_api_token_expired() {
                    println!("WARNING (token expired)");
                } else {
                    println!("OK (logged in)");
                }
            },
            Ok(None) => {
                println!("N/A (not logged in)");
            },
            Err(e) => {
                println!("ERROR - {e}");
                issues += 1;
            },
        },
        Err(e) => {
            println!("ERROR - {e}");
            issues += 1;
        },
    }

    // Summary
    println!();
    if issues == 0 {
        println!("All checks passed!");
    } else {
        println!("{issues} issue(s) found.");
    }

    Ok(())
}

/// Handles the `desk history` command.
///
/// Shows recent workspace switches.
pub fn handle_history(limit: usize, repo_only: bool) -> Result<()> {
    let state = DeskState::load()?;

    let entries = if repo_only {
        let repo_path = std::env::current_dir()?;
        state.get_history_for_repo(&repo_path, limit)
    } else {
        state.get_history(limit).iter().collect()
    };

    if entries.is_empty() {
        println!("No workspace history found.");
        return Ok(());
    }

    println!("Recent workspace switches:\n");

    for entry in entries {
        let time_ago = format_time_ago(entry.timestamp);
        println!("  {} ({})", entry.workspace, time_ago);
        if !repo_only {
            println!("    {}", entry.repo_path);
        }
    }

    Ok(())
}

/// Format a timestamp as relative time.
fn format_time_ago(timestamp: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(timestamp);

    if duration.num_days() > 0 {
        format!("{} day(s) ago", duration.num_days())
    } else if duration.num_hours() > 0 {
        format!("{} hour(s) ago", duration.num_hours())
    } else if duration.num_minutes() > 0 {
        format!("{} minute(s) ago", duration.num_minutes())
    } else {
        "just now".to_string()
    }
}

/// Handles the `desk config` command.
///
/// Views or modifies configuration.
pub fn handle_config(key: Option<&str>, value: Option<&str>, list: bool) -> Result<()> {
    let config = crate::config::load_config()?;

    if list || (key.is_none() && value.is_none()) {
        // List all config values
        println!("Current configuration:\n");
        println!("  api.base_url = {}", config.api.base_url);
        println!("  api.timeout_secs = {}", config.api.timeout_secs);
        println!();
        println!(
            "Config file: {}",
            crate::config::paths::config_file()?.display()
        );
        return Ok(());
    }

    if let Some(k) = key {
        if let Some(v) = value {
            // Set config value
            println!("Setting configuration is not yet implemented.");
            println!(
                "Edit the config file directly: {}",
                crate::config::paths::config_file()?.display()
            );
            println!();
            println!("To set {k} = {v}");
        } else {
            // Get config value
            match k {
                "api.base_url" => println!("{}", config.api.base_url),
                "api.timeout_secs" => println!("{}", config.api.timeout_secs),
                _ => {
                    println!("Unknown configuration key: {k}");
                    println!("\nAvailable keys:");
                    println!("  api.base_url");
                    println!("  api.timeout_secs");
                },
            }
        }
    }

    Ok(())
}

/// Handles the `desk tag` command.
///
/// Manages workspace tags.
pub fn handle_tag(name: &str, command: crate::cli::TagCommands) -> Result<()> {
    use crate::cli::TagCommands;

    let store = FileWorkspaceStore::new()?;

    let Some(mut workspace) = store.load(name)? else {
        println!("Workspace '{name}' not found.");
        std::process::exit(1);
    };

    match command {
        TagCommands::Add { tags } => {
            for tag in &tags {
                if !workspace.metadata.tags.contains(tag) {
                    workspace.metadata.tags.push(tag.clone());
                }
            }
            workspace.metadata.tags.sort();
            workspace.touch();
            store.save(&workspace, true)?;
            println!("Added {} tag(s) to '{name}'.", tags.len());
        },
        TagCommands::Remove { tags } => {
            let before = workspace.metadata.tags.len();
            workspace.metadata.tags.retain(|t| !tags.contains(t));
            let removed = before - workspace.metadata.tags.len();
            workspace.touch();
            store.save(&workspace, true)?;
            println!("Removed {removed} tag(s) from '{name}'.");
        },
        TagCommands::List => {
            if workspace.metadata.tags.is_empty() {
                println!("No tags on workspace '{name}'.");
            } else {
                println!("Tags on '{name}':");
                for tag in &workspace.metadata.tags {
                    println!("  {tag}");
                }
            }
        },
        TagCommands::Clear => {
            let count = workspace.metadata.tags.len();
            workspace.metadata.tags.clear();
            workspace.touch();
            store.save(&workspace, true)?;
            println!("Cleared {count} tag(s) from '{name}'.");
        },
    }

    Ok(())
}

/// Handles the `desk archive <name>` command.
///
/// Archives a workspace (hides from default list).
pub fn handle_archive(name: &str) -> Result<()> {
    let store = FileWorkspaceStore::new()?;

    let Some(mut workspace) = store.load(name)? else {
        println!("Workspace '{name}' not found.");
        std::process::exit(1);
    };

    if workspace.metadata.archived {
        println!("Workspace '{name}' is already archived.");
        return Ok(());
    }

    workspace.metadata.archived = true;
    workspace.touch();
    store.save(&workspace, true)?;

    println!("Archived workspace '{name}'.");
    println!("Use 'desk list --archived' to see archived workspaces.");

    Ok(())
}

/// Handles the `desk unarchive <name>` command.
///
/// Unarchives a workspace (restores to default list).
pub fn handle_unarchive(name: &str) -> Result<()> {
    let store = FileWorkspaceStore::new()?;

    let Some(mut workspace) = store.load(name)? else {
        println!("Workspace '{name}' not found.");
        std::process::exit(1);
    };

    if !workspace.metadata.archived {
        println!("Workspace '{name}' is not archived.");
        return Ok(());
    }

    workspace.metadata.archived = false;
    workspace.touch();
    store.save(&workspace, true)?;

    println!("Unarchived workspace '{name}'.");

    Ok(())
}

/// Handles the `desk alias` command.
///
/// Manages workspace aliases.
pub fn handle_alias(command: crate::cli::AliasCommands) -> Result<()> {
    use crate::cli::AliasCommands;

    let mut state = DeskState::load()?;

    match command {
        AliasCommands::Set { alias, workspace } => {
            // Verify workspace exists
            let store = FileWorkspaceStore::new()?;
            if !store.exists(&workspace)? {
                println!("Workspace '{workspace}' not found.");
                std::process::exit(1);
            }

            state.set_alias(&alias, &workspace);
            state.save()?;
            println!("Created alias '{alias}' -> '{workspace}'.");
        },
        AliasCommands::Remove { alias } => {
            if state.remove_alias(&alias) {
                state.save()?;
                println!("Removed alias '{alias}'.");
            } else {
                println!("Alias '{alias}' not found.");
            }
        },
        AliasCommands::List => {
            let aliases = state.get_aliases();
            if aliases.is_empty() {
                println!("No aliases defined.");
                println!("\nCreate one with: desk alias set <alias> <workspace>");
            } else {
                println!("Workspace aliases:\n");
                for (alias, workspace) in aliases {
                    println!("  {alias} -> {workspace}");
                }
            }
        },
    }

    Ok(())
}

/// Handles the `desk diff <ws1> <ws2>` command.
///
/// Compares two workspaces.
#[allow(clippy::too_many_lines)]
pub fn handle_diff(workspace1: &str, workspace2: &str) -> Result<()> {
    let store = FileWorkspaceStore::new()?;

    let Some(ws1) = store.load(workspace1)? else {
        println!("Workspace '{workspace1}' not found.");
        std::process::exit(1);
    };

    let Some(ws2) = store.load(workspace2)? else {
        println!("Workspace '{workspace2}' not found.");
        std::process::exit(1);
    };

    println!("Comparing workspaces:\n");
    println!("  {workspace1} vs {workspace2}\n");

    // Branch comparison
    if ws1.branch == ws2.branch {
        println!("  Branch:      {} (same)", ws1.branch);
    } else {
        println!("  Branch:      {} vs {}", ws1.branch, ws2.branch);
    }

    // Commit comparison
    if ws1.commit_sha == ws2.commit_sha {
        println!("  Commit:      {} (same)", &ws1.commit_sha[..7]);
    } else {
        println!(
            "  Commit:      {} vs {}",
            &ws1.commit_sha[..7],
            &ws2.commit_sha[..7]
        );
    }

    // Repository path
    if ws1.repo_path == ws2.repo_path {
        println!("  Repository:  {} (same)", ws1.repo_path.display());
    } else {
        println!(
            "  Repository:  {} vs {}",
            ws1.repo_path.display(),
            ws2.repo_path.display()
        );
    }

    // Stash
    match (&ws1.stash_name, &ws2.stash_name) {
        (Some(s1), Some(s2)) if s1 == s2 => {
            println!("  Stash:       {s1} (same)");
        },
        (Some(s1), Some(s2)) => {
            println!("  Stash:       {s1} vs {s2}");
        },
        (Some(s1), None) => {
            println!("  Stash:       {s1} vs (none)");
        },
        (None, Some(s2)) => {
            println!("  Stash:       (none) vs {s2}");
        },
        (None, None) => {
            println!("  Stash:       (none)");
        },
    }

    // Tags
    println!();
    let tags1: std::collections::HashSet<_> = ws1.metadata.tags.iter().collect();
    let tags2: std::collections::HashSet<_> = ws2.metadata.tags.iter().collect();

    let only_in_1: Vec<_> = tags1.difference(&tags2).collect();
    let only_in_2: Vec<_> = tags2.difference(&tags1).collect();
    let common: Vec<_> = tags1.intersection(&tags2).collect();

    if !common.is_empty() {
        println!(
            "  Common tags: {}",
            common
                .iter()
                .map(|t| t.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !only_in_1.is_empty() {
        println!(
            "  Only in {}: {}",
            workspace1,
            only_in_1
                .iter()
                .map(|t| t.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !only_in_2.is_empty() {
        println!(
            "  Only in {}: {}",
            workspace2,
            only_in_2
                .iter()
                .map(|t| t.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    // Timestamps
    println!();
    println!(
        "  Created:     {} vs {}",
        ws1.created_at.format("%Y-%m-%d %H:%M"),
        ws2.created_at.format("%Y-%m-%d %H:%M")
    );
    println!(
        "  Updated:     {} vs {}",
        ws1.updated_at.format("%Y-%m-%d %H:%M"),
        ws2.updated_at.format("%Y-%m-%d %H:%M")
    );

    Ok(())
}

/// Handles the `desk stats` command.
///
/// Shows workspace usage statistics.
pub fn handle_stats() -> Result<()> {
    let store = FileWorkspaceStore::new()?;
    let workspaces = store.list()?;
    let state = DeskState::load()?;

    if workspaces.is_empty() {
        println!("No workspaces found.");
        return Ok(());
    }

    println!("Workspace Statistics\n");

    // Total counts
    let total = workspaces.len();
    let archived = workspaces.iter().filter(|w| w.metadata.archived).count();
    let synced = workspaces
        .iter()
        .filter(|w| w.metadata.remote_id.is_some())
        .count();

    println!("  Total workspaces:    {total}");
    println!("  Active:              {}", total - archived);
    println!("  Archived:            {archived}");
    println!("  Synced to cloud:     {synced}");

    // Most used workspaces (by open count)
    let mut by_usage: Vec<_> = workspaces
        .iter()
        .filter(|w| w.metadata.open_count > 0)
        .collect();
    by_usage.sort_by(|a, b| b.metadata.open_count.cmp(&a.metadata.open_count));

    if !by_usage.is_empty() {
        println!("\n  Most used:");
        for ws in by_usage.iter().take(5) {
            println!("    {} ({} opens)", ws.name, ws.metadata.open_count);
        }
    }

    // Most time spent
    let mut by_time: Vec<_> = workspaces
        .iter()
        .filter(|w| w.metadata.total_time_secs > 0)
        .collect();
    by_time.sort_by(|a, b| b.metadata.total_time_secs.cmp(&a.metadata.total_time_secs));

    if !by_time.is_empty() {
        println!("\n  Most time spent:");
        for ws in by_time.iter().take(5) {
            let hours = ws.metadata.total_time_secs / 3600;
            let mins = (ws.metadata.total_time_secs % 3600) / 60;
            println!("    {} ({}h {}m)", ws.name, hours, mins);
        }
    }

    // Recently used
    let mut by_recent: Vec<_> = workspaces
        .iter()
        .filter(|w| w.metadata.last_opened_at.is_some())
        .collect();
    by_recent.sort_by(|a, b| b.metadata.last_opened_at.cmp(&a.metadata.last_opened_at));

    if !by_recent.is_empty() {
        println!("\n  Recently used:");
        for ws in by_recent.iter().take(5) {
            if let Some(opened) = ws.metadata.last_opened_at {
                let ago = format_time_ago(opened);
                println!("    {} ({})", ws.name, ago);
            }
        }
    }

    // Tags breakdown
    let mut tag_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for ws in &workspaces {
        for tag in &ws.metadata.tags {
            *tag_counts.entry(tag.as_str()).or_insert(0) += 1;
        }
    }

    if !tag_counts.is_empty() {
        println!("\n  Tags:");
        let mut sorted_tags: Vec<_> = tag_counts.iter().collect();
        sorted_tags.sort_by(|a, b| b.1.cmp(a.1));
        for (tag, count) in sorted_tags.iter().take(10) {
            println!("    {tag} ({count})");
        }
    }

    // History stats
    println!("\n  History entries:     {}", state.history.len());
    println!("  Aliases defined:     {}", state.aliases.len());

    Ok(())
}

/// Handles the `desk hooks` command.
///
/// Manages workspace switch hooks.
pub fn handle_hooks(command: crate::cli::HookCommands) -> Result<()> {
    use crate::cli::{HookCommands, HookType};

    let mut state = DeskState::load()?;

    match command {
        HookCommands::Add { hook_type, command } => {
            match hook_type {
                HookType::PreSwitch => {
                    state.add_pre_switch_hook(command.clone());
                    println!("Added pre-switch hook: {command}");
                },
                HookType::PostSwitch => {
                    state.add_post_switch_hook(command.clone());
                    println!("Added post-switch hook: {command}");
                },
            }
            state.save()?;
        },
        HookCommands::Remove { hook_type, index } => {
            let removed = match hook_type {
                HookType::PreSwitch => state.remove_pre_switch_hook(index),
                HookType::PostSwitch => state.remove_post_switch_hook(index),
            };
            if removed {
                state.save()?;
                println!("Removed hook at index {index}.");
            } else {
                println!("No hook found at index {index}.");
            }
        },
        HookCommands::List => {
            println!("Workspace hooks:\n");

            if state.pre_switch_hooks.is_empty() && state.post_switch_hooks.is_empty() {
                println!("  No hooks configured.");
                println!("\n  Add one with: desk hooks add pre-switch \"your-command\"");
                return Ok(());
            }

            if !state.pre_switch_hooks.is_empty() {
                println!("  Pre-switch hooks:");
                for (i, cmd) in state.pre_switch_hooks.iter().enumerate() {
                    println!("    [{i}] {cmd}");
                }
            }

            if !state.post_switch_hooks.is_empty() {
                println!("  Post-switch hooks:");
                for (i, cmd) in state.post_switch_hooks.iter().enumerate() {
                    println!("    [{i}] {cmd}");
                }
            }
        },
        HookCommands::Clear => {
            state.clear_hooks();
            state.save()?;
            println!("Cleared all hooks.");
        },
    }

    Ok(())
}

/// Handles the `desk watch` command.
///
/// Watches for changes and auto-saves workspace state.
pub fn handle_watch(interval: u64, name: Option<String>) -> Result<()> {
    use std::thread;
    use std::time::Duration;

    let store = FileWorkspaceStore::new()?;
    let repo_path = std::env::current_dir()?;

    // Determine workspace name
    let workspace_name = if let Some(n) = name {
        n
    } else {
        let state = DeskState::load()?;
        let Some(n) = state.get_current(&repo_path) else {
            println!(
                "No active workspace. Specify a name with --name or open a workspace first."
            );
            std::process::exit(1);
        };
        n.clone()
    };

    // Verify workspace exists
    if !store.exists(&workspace_name)? {
        println!("Workspace '{workspace_name}' not found.");
        std::process::exit(1);
    }

    println!("Watching workspace '{workspace_name}'...");
    println!("Auto-saving every {interval} seconds. Press Ctrl+C to stop.\n");

    loop {
        thread::sleep(Duration::from_secs(interval));

        // Reload and update workspace
        if let Some(mut workspace) = store.load(&workspace_name)? {
            let git = Git2Operations::from_current_dir()?;
            let status = git.status()?;

            // Capture values before moving
            let has_changes = status.has_changes();
            let total_changes = status.total_changes();

            // Update workspace state
            workspace.branch = status.branch;
            workspace.commit_sha = status.commit_sha;
            workspace.metadata.was_dirty = Some(has_changes);
            #[allow(clippy::cast_possible_truncation)]
            {
                workspace.metadata.uncommitted_files = Some(total_changes as u32);
            }
            workspace.touch();

            store.save(&workspace, true)?;

            let now = chrono::Local::now();
            println!(
                "[{}] Saved workspace state (branch: {}, {} changes)",
                now.format("%H:%M:%S"),
                workspace.branch,
                total_changes
            );
        } else {
            println!("Workspace '{workspace_name}' was deleted. Stopping watch.");
            break;
        }
    }

    Ok(())
}

/// Handles the `desk note` command.
///
/// Manages workspace notes.
pub fn handle_note(name: &str, command: crate::cli::NoteCommands) -> Result<()> {
    use crate::cli::NoteCommands;
    use crate::workspace::types::WorkspaceNote;

    let store = FileWorkspaceStore::new()?;

    let Some(mut workspace) = store.load(name)? else {
        println!("Workspace '{name}' not found.");
        std::process::exit(1);
    };

    match command {
        NoteCommands::Add { text } => {
            let note = WorkspaceNote {
                text,
                created_at: chrono::Utc::now(),
            };
            workspace.metadata.notes.push(note);
            workspace.touch();
            store.save(&workspace, true)?;
            println!("Added note to '{name}'.");
        },
        NoteCommands::List => {
            if workspace.metadata.notes.is_empty() {
                println!("No notes on workspace '{name}'.");
            } else {
                println!("Notes on '{name}':\n");
                for (i, note) in workspace.metadata.notes.iter().enumerate() {
                    let time = note.created_at.format("%Y-%m-%d %H:%M");
                    println!("  [{}] {} ({})", i, note.text, time);
                }
            }
        },
        NoteCommands::Clear => {
            let count = workspace.metadata.notes.len();
            workspace.metadata.notes.clear();
            workspace.touch();
            store.save(&workspace, true)?;
            println!("Cleared {count} note(s) from '{name}'.");
        },
    }

    Ok(())
}

/// Handles the `desk bulk` command.
///
/// Bulk operations on multiple workspaces.
pub fn handle_bulk(command: crate::cli::BulkCommands) -> Result<()> {
    use crate::cli::BulkCommands;

    let store = FileWorkspaceStore::new()?;

    match command {
        BulkCommands::Delete { names, yes } => {
            if !yes {
                print!("Delete {} workspace(s)? [y/N] ", names.len());
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;

                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Cancelled.");
                    return Ok(());
                }
            }

            let mut deleted = 0;
            for name in &names {
                if store.delete(name)? {
                    println!("Deleted '{name}'.");
                    deleted += 1;
                } else {
                    println!("Workspace '{name}' not found.");
                }
            }
            println!("\nDeleted {deleted} workspace(s).");
        },
        BulkCommands::Tag { names, tags } => {
            let mut tagged = 0;
            for name in &names {
                if let Some(mut ws) = store.load(name)? {
                    for tag in &tags {
                        if !ws.metadata.tags.contains(tag) {
                            ws.metadata.tags.push(tag.clone());
                        }
                    }
                    ws.metadata.tags.sort();
                    ws.touch();
                    store.save(&ws, true)?;
                    tagged += 1;
                } else {
                    println!("Workspace '{name}' not found.");
                }
            }
            println!("Added {} tag(s) to {tagged} workspace(s).", tags.len());
        },
        BulkCommands::Archive { names } => {
            let mut archived = 0;
            for name in &names {
                if let Some(mut ws) = store.load(name)? {
                    if ws.metadata.archived {
                        println!("'{name}' already archived.");
                    } else {
                        ws.metadata.archived = true;
                        ws.touch();
                        store.save(&ws, true)?;
                        println!("Archived '{name}'.");
                        archived += 1;
                    }
                } else {
                    println!("Workspace '{name}' not found.");
                }
            }
            println!("\nArchived {archived} workspace(s).");
        },
        BulkCommands::Export { names, output } => {
            let output_dir = std::path::Path::new(&output);
            if !output_dir.exists() {
                std::fs::create_dir_all(output_dir)?;
            }

            let mut exported = 0;
            for name in &names {
                if let Some(ws) = store.load(name)? {
                    let file_path = output_dir.join(format!("{name}.json"));
                    let json = serde_json::to_string_pretty(&ws)?;
                    std::fs::write(&file_path, json)?;
                    println!("Exported '{name}' to {}", file_path.display());
                    exported += 1;
                } else {
                    println!("Workspace '{name}' not found.");
                }
            }
            println!("\nExported {exported} workspace(s).");
        },
    }

    Ok(())
}

/// Handles interactive workspace selection.
///
/// Shows a numbered list and lets user select by number.
pub fn handle_interactive_open() -> Result<()> {
    let store = FileWorkspaceStore::new()?;
    let workspaces = store.list()?;

    // Filter out archived workspaces
    let active: Vec<_> = workspaces.iter().filter(|w| !w.metadata.archived).collect();

    if active.is_empty() {
        println!("No workspaces available.");
        println!("\nCreate one with: desk open <name>");
        return Ok(());
    }

    println!("Select a workspace:\n");

    for (i, ws) in active.iter().enumerate() {
        let tags = if ws.metadata.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", ws.metadata.tags.join(", "))
        };
        println!("  {:>2}) {}{}", i + 1, ws.name, tags);
        println!("      Branch: {}", ws.branch);
    }

    print!("\nEnter number (1-{}): ", active.len());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let selection: usize = match input.trim().parse() {
        Ok(n) if n >= 1 && n <= active.len() => n,
        _ => {
            println!("Invalid selection.");
            std::process::exit(1);
        },
    };

    let selected = active[selection - 1];
    let mut git = Git2Operations::from_current_dir()?;

    restore_workspace(&mut git, selected)?;

    // Track current workspace
    let repo_path = std::env::current_dir()?;
    let mut state = DeskState::load()?;
    state.set_current(&repo_path, &selected.name);
    state.record_workspace_opened(&repo_path);
    state.save()?;

    // Update workspace stats
    let mut ws = selected.clone();
    ws.metadata.open_count += 1;
    ws.metadata.last_opened_at = Some(chrono::Utc::now());
    store.save(&ws, true)?;

    println!("\nSwitched to workspace '{}'.", selected.name);

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
