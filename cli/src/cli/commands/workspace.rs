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
    let repo_path = std::env::current_dir()?;

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

    // Track current workspace in state
    let mut state = DeskState::load()?;
    state.set_current(&repo_path, name);
    state.save()?;

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
    let workspace = match store.load(name)? {
        Some(ws) => ws,
        None => {
            println!("Workspace '{name}' not found.");
            std::process::exit(1);
        }
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
                }
                Err(DeskError::SubscriptionRequired) => {
                    println!("Warning: Could not delete from cloud (Pro subscription required).");
                }
                Err(e) => {
                    println!("Warning: Failed to delete from cloud: {e}");
                }
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
    let mut workspace = match store.load(name)? {
        Some(ws) => ws,
        None => {
            println!("Workspace '{name}' not found.");
            std::process::exit(1);
        }
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
                }
                Err(DeskError::SubscriptionRequired) => {
                    println!("Warning: Could not rename on cloud (Pro subscription required).");
                }
                Err(e) => {
                    println!("Warning: Failed to rename on cloud: {e}");
                    println!("Local rename will still proceed.");
                }
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

    let workspace = match store.load(name)? {
        Some(ws) => ws,
        None => {
            println!("Workspace '{name}' not found.");
            std::process::exit(1);
        }
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
    let workspace = match store.load(name)? {
        Some(ws) => ws,
        None => {
            println!("Workspace '{name}' not found.");
            std::process::exit(1);
        }
    };

    // Check if target already exists
    if store.exists(new_name)? {
        println!("Workspace '{new_name}' already exists.");
        std::process::exit(1);
    }

    // Create clone with new name
    let mut cloned = workspace.clone();
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
    let mut workspace = match store.load(name)? {
        Some(ws) => ws,
        None => {
            println!("Workspace '{name}' not found.");
            std::process::exit(1);
        }
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
                }
                Err(DeskError::SubscriptionRequired) => {
                    println!("Warning: Could not update cloud (Pro subscription required).");
                }
                Err(e) => {
                    println!("Warning: Failed to update cloud: {e}");
                }
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

    let workspace = match store.load(name)? {
        Some(ws) => ws,
        None => {
            println!("Workspace '{name}' not found.");
            std::process::exit(1);
        }
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

    if execute {
        println!();
        // Drop stashes in reverse order to preserve indices
        let mut dropped = 0;
        for stash in orphaned.iter().rev() {
            if git.stash_drop(stash.index).is_ok() {
                dropped += 1;
            }
        }
        println!("Dropped {dropped} stash(es).");
    } else {
        println!();
        println!("Run with --execute to delete these stashes.");
    }

    Ok(())
}

/// Handles the `desk prompt` command.
///
/// Outputs the current workspace name for use in shell prompts.
/// If no workspace is active, outputs nothing.
///
/// # Errors
///
/// Returns an error if state cannot be loaded.
pub fn handle_prompt() -> Result<()> {
    let repo_path = match std::env::current_dir() {
        Ok(p) => p,
        Err(_) => return Ok(()), // Silently fail for prompt
    };

    let state = DeskState::load().unwrap_or_default();

    if let Some(workspace_name) = state.get_current(&repo_path) {
        print!("{workspace_name}");
    }

    Ok(())
}

/// Handles the `desk init <shell>` command.
///
/// Generates shell integration script for the specified shell.
///
/// # Arguments
///
/// * `shell` - The shell type to generate script for
pub fn handle_init(shell: ShellType) -> Result<()> {
    let script = match shell {
        ShellType::Bash => BASH_INIT_SCRIPT,
        ShellType::Zsh => ZSH_INIT_SCRIPT,
        ShellType::Fish => FISH_INIT_SCRIPT,
    };

    println!("{script}");

    Ok(())
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
                    || ws.description
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
pub fn handle_completions(shell: ShellType) -> Result<()> {
    use clap::CommandFactory;
    use clap_complete::{generate, Shell};

    let mut cmd = crate::cli::Cli::command();
    let shell = match shell {
        ShellType::Bash => Shell::Bash,
        ShellType::Zsh => Shell::Zsh,
        ShellType::Fish => Shell::Fish,
    };

    generate(shell, &mut cmd, "desk", &mut std::io::stdout());

    Ok(())
}

/// Handles the `desk doctor` command.
///
/// Checks desk installation and diagnoses issues.
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
        }
        Err(e) => {
            println!("ERROR - {e}");
            issues += 1;
        }
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
        }
        Err(e) => {
            println!("ERROR - {e}");
            issues += 1;
        }
    }

    // Check 3: Workspaces directory
    print!("  Workspaces directory: ");
    match crate::config::paths::workspaces_dir() {
        Ok(path) => {
            if path.exists() {
                // Count workspaces
                let store = FileWorkspaceStore::new()?;
                let count = store.list()?.len();
                println!("OK ({} workspace(s))", count);
            } else {
                println!("OK (will be created)");
            }
        }
        Err(e) => {
            println!("ERROR - {e}");
            issues += 1;
        }
    }

    // Check 4: Git access
    print!("  Git repository: ");
    match Git2Operations::from_current_dir() {
        Ok(git) => match git.status() {
            Ok(status) => {
                println!("OK (branch: {}, {})",
                    status.branch,
                    if status.is_dirty { "dirty" } else { "clean" }
                );
            }
            Err(e) => {
                println!("WARNING - {e}");
            }
        },
        Err(_) => {
            println!("N/A (not in a git repository)");
        }
    }

    // Check 5: Config file
    print!("  Configuration: ");
    match crate::config::load_config() {
        Ok(config) => {
            println!("OK (API: {})", config.api.base_url);
        }
        Err(e) => {
            println!("ERROR - {e}");
            issues += 1;
        }
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
            }
            Ok(None) => {
                println!("N/A (not logged in)");
            }
            Err(e) => {
                println!("ERROR - {e}");
                issues += 1;
            }
        },
        Err(e) => {
            println!("ERROR - {e}");
            issues += 1;
        }
    }

    // Summary
    println!();
    if issues == 0 {
        println!("All checks passed!");
    } else {
        println!("{} issue(s) found.", issues);
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
pub fn handle_config(key: Option<String>, value: Option<String>, list: bool) -> Result<()> {
    let config = crate::config::load_config()?;

    if list || (key.is_none() && value.is_none()) {
        // List all config values
        println!("Current configuration:\n");
        println!("  api.base_url = {}", config.api.base_url);
        println!("  api.timeout_secs = {}", config.api.timeout_secs);
        println!();
        println!("Config file: {}", crate::config::paths::config_file()?.display());
        return Ok(());
    }

    if let Some(ref k) = key {
        if let Some(ref v) = value {
            // Set config value
            println!("Setting configuration is not yet implemented.");
            println!("Edit the config file directly: {}", crate::config::paths::config_file()?.display());
            println!();
            println!("To set {k} = {v}");
        } else {
            // Get config value
            match k.as_str() {
                "api.base_url" => println!("{}", config.api.base_url),
                "api.timeout_secs" => println!("{}", config.api.timeout_secs),
                _ => {
                    println!("Unknown configuration key: {k}");
                    println!("\nAvailable keys:");
                    println!("  api.base_url");
                    println!("  api.timeout_secs");
                }
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

    let mut workspace = match store.load(name)? {
        Some(ws) => ws,
        None => {
            println!("Workspace '{name}' not found.");
            std::process::exit(1);
        }
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
        }
        TagCommands::Remove { tags } => {
            let before = workspace.metadata.tags.len();
            workspace.metadata.tags.retain(|t| !tags.contains(t));
            let removed = before - workspace.metadata.tags.len();
            workspace.touch();
            store.save(&workspace, true)?;
            println!("Removed {removed} tag(s) from '{name}'.");
        }
        TagCommands::List => {
            if workspace.metadata.tags.is_empty() {
                println!("No tags on workspace '{name}'.");
            } else {
                println!("Tags on '{name}':");
                for tag in &workspace.metadata.tags {
                    println!("  {tag}");
                }
            }
        }
        TagCommands::Clear => {
            let count = workspace.metadata.tags.len();
            workspace.metadata.tags.clear();
            workspace.touch();
            store.save(&workspace, true)?;
            println!("Cleared {count} tag(s) from '{name}'.");
        }
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
