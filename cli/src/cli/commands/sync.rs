//! Sync command handlers for the desk CLI.
//!
//! This module implements workspace synchronization with the cloud:
//! - [`handle_sync_push`] - Push workspaces to the cloud (`desk sync push`)
//! - [`handle_sync_pull`] - Pull workspaces from the cloud (`desk sync pull`)
//! - [`handle_sync_status`] - Show sync status (`desk sync status`)
//!
//! Sync is a Pro feature that requires authentication.

use chrono::Utc;

use crate::client::{DeskApiClient, RemoteWorkspace, WorkspaceState, WorkspaceStateMetadata};
use crate::config::load_config;
use crate::error::{DeskError, Result};
use crate::workspace::{FileWorkspaceStore, Workspace, WorkspaceStore};

/// Handles the `desk sync push` command.
///
/// Pushes local workspace(s) to the cloud. If no name is specified,
/// pushes all local workspaces.
///
/// # Arguments
///
/// * `name` - Optional workspace name to push (all if omitted)
/// * `force` - Force push even if remote has newer changes
///
/// # Errors
///
/// Returns an error if:
/// - Not authenticated
/// - Pro subscription required
/// - Network/API errors
pub async fn handle_sync_push(name: Option<String>, force: bool) -> Result<()> {
    let config = load_config()?;
    let client = DeskApiClient::new(&config.api)?;

    // Load credentials
    if !client.load_credentials().await? {
        return Err(DeskError::NotAuthenticated);
    }

    let store = FileWorkspaceStore::new()?;

    // Get workspaces to push
    let workspaces = if let Some(ref name) = name {
        match store.load(name)? {
            Some(ws) => vec![ws],
            None => {
                println!("Workspace '{name}' not found locally.");
                return Ok(());
            }
        }
    } else {
        store.list()?
    };

    if workspaces.is_empty() {
        println!("No local workspaces to push.");
        return Ok(());
    }

    // Fetch remote workspaces for comparison
    let remote_workspaces = match client.list_workspaces().await {
        Ok(ws) => ws,
        Err(DeskError::SubscriptionRequired) => {
            return Err(DeskError::SubscriptionRequired);
        }
        Err(e) => return Err(e),
    };

    let remote_by_name: std::collections::HashMap<_, _> = remote_workspaces
        .into_iter()
        .map(|ws| (ws.name.clone(), ws))
        .collect();

    let mut pushed = 0;
    let mut skipped = 0;

    for mut workspace in workspaces {
        let state = workspace_to_state(&workspace);

        // Check if remote exists
        if let Some(remote) = remote_by_name.get(&workspace.name) {
            let local_version = workspace.metadata.remote_version.unwrap_or(0);

            // Check for conflict
            if remote.version > local_version && !force {
                println!(
                    "  {} - skipped (remote has newer changes, use --force to overwrite)",
                    workspace.name
                );
                skipped += 1;
                continue;
            }

            // Update existing
            match client
                .update_workspace(
                    &remote.id,
                    None, // keep name
                    workspace.description.as_deref(),
                    Some(&state),
                    if force { remote.version } else { local_version },
                )
                .await
            {
                Ok(updated) => {
                    // Update local metadata
                    workspace.metadata.remote_id = Some(updated.id);
                    workspace.metadata.remote_version = Some(updated.version);
                    workspace.metadata.last_synced_at = Some(Utc::now());
                    store.save(&workspace, true)?;

                    println!("  {} - pushed (updated)", workspace.name);
                    pushed += 1;
                }
                Err(e) => {
                    println!("  {} - failed: {e}", workspace.name);
                    skipped += 1;
                }
            }
        } else {
            // Create new
            match client
                .create_workspace(&workspace.name, workspace.description.as_deref(), &state)
                .await
            {
                Ok(created) => {
                    // Update local metadata
                    workspace.metadata.remote_id = Some(created.id);
                    workspace.metadata.remote_version = Some(created.version);
                    workspace.metadata.last_synced_at = Some(Utc::now());
                    store.save(&workspace, true)?;

                    println!("  {} - pushed (new)", workspace.name);
                    pushed += 1;
                }
                Err(e) => {
                    println!("  {} - failed: {e}", workspace.name);
                    skipped += 1;
                }
            }
        }
    }

    println!();
    println!("Pushed: {pushed}, Skipped: {skipped}");

    Ok(())
}

/// Handles the `desk sync pull` command.
///
/// Pulls workspace(s) from the cloud. If no name is specified,
/// pulls all remote workspaces.
///
/// # Arguments
///
/// * `name` - Optional workspace name to pull (all if omitted)
/// * `force` - Force pull even if local has newer changes
///
/// # Errors
///
/// Returns an error if:
/// - Not authenticated
/// - Pro subscription required
/// - Network/API errors
pub async fn handle_sync_pull(name: Option<String>, force: bool) -> Result<()> {
    let config = load_config()?;
    let client = DeskApiClient::new(&config.api)?;

    // Load credentials
    if !client.load_credentials().await? {
        return Err(DeskError::NotAuthenticated);
    }

    let store = FileWorkspaceStore::new()?;

    // Fetch remote workspaces
    let remote_workspaces = match client.list_workspaces().await {
        Ok(ws) => ws,
        Err(DeskError::SubscriptionRequired) => {
            return Err(DeskError::SubscriptionRequired);
        }
        Err(e) => return Err(e),
    };

    // Filter by name if specified
    let remotes: Vec<_> = if let Some(ref name) = name {
        remote_workspaces
            .into_iter()
            .filter(|ws| ws.name == *name)
            .collect()
    } else {
        remote_workspaces
    };

    if remotes.is_empty() {
        if name.is_some() {
            println!("Workspace not found in the cloud.");
        } else {
            println!("No remote workspaces to pull.");
        }
        return Ok(());
    }

    let mut pulled = 0;
    let mut skipped = 0;

    for remote in remotes {
        // Check if local exists
        let local = store.load(&remote.name)?;

        if let Some(local_ws) = &local {
            let local_version = local_ws.metadata.remote_version.unwrap_or(0);

            // Check for conflict - local has changes that haven't been pushed
            if local_version > 0 && local_version < remote.version && !force {
                // Remote is newer, safe to pull
            } else if local_version > remote.version && !force {
                println!(
                    "  {} - skipped (local has newer changes, use --force to overwrite)",
                    remote.name
                );
                skipped += 1;
                continue;
            }
        }

        // Create/update local workspace from remote
        let workspace = state_to_workspace(&remote);
        match store.save(&workspace, true) {
            Ok(()) => {
                if local.is_some() {
                    println!("  {} - pulled (updated)", remote.name);
                } else {
                    println!("  {} - pulled (new)", remote.name);
                }
                pulled += 1;
            }
            Err(e) => {
                println!("  {} - failed: {e}", remote.name);
                skipped += 1;
            }
        }
    }

    println!();
    println!("Pulled: {pulled}, Skipped: {skipped}");

    Ok(())
}

/// Handles the `desk sync status` command.
///
/// Shows the sync status of all workspaces, comparing local and remote.
///
/// # Errors
///
/// Returns an error if:
/// - Not authenticated
/// - Pro subscription required
/// - Network/API errors
pub async fn handle_sync_status() -> Result<()> {
    let config = load_config()?;
    let client = DeskApiClient::new(&config.api)?;

    // Load credentials
    if !client.load_credentials().await? {
        return Err(DeskError::NotAuthenticated);
    }

    let store = FileWorkspaceStore::new()?;

    // Get local workspaces
    let local_workspaces = store.list()?;
    let local_by_name: std::collections::HashMap<_, _> = local_workspaces
        .into_iter()
        .map(|ws| (ws.name.clone(), ws))
        .collect();

    // Fetch remote workspaces
    let remote_workspaces = match client.list_workspaces().await {
        Ok(ws) => ws,
        Err(DeskError::SubscriptionRequired) => {
            println!("Sync requires a Pro subscription.");
            println!("Visit https://getdesk.dev/pricing to upgrade.\n");

            // Still show local workspaces
            if !local_by_name.is_empty() {
                println!("Local workspaces (not synced):");
                for name in local_by_name.keys() {
                    println!("  {name}  [local only]");
                }
            }
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    let remote_by_name: std::collections::HashMap<_, _> = remote_workspaces
        .into_iter()
        .map(|ws| (ws.name.clone(), ws))
        .collect();

    // Collect all workspace names
    let mut all_names: Vec<_> = local_by_name
        .keys()
        .chain(remote_by_name.keys())
        .cloned()
        .collect();
    all_names.sort();
    all_names.dedup();

    if all_names.is_empty() {
        println!("No workspaces found.");
        return Ok(());
    }

    println!("Workspace sync status:\n");

    for name in all_names {
        let local = local_by_name.get(&name);
        let remote = remote_by_name.get(&name);

        match (local, remote) {
            (Some(local_ws), Some(remote_ws)) => {
                let local_version = local_ws.metadata.remote_version.unwrap_or(0);

                let status = if local_version == remote_ws.version {
                    "synced"
                } else if local_version > remote_ws.version {
                    "local ahead"
                } else {
                    "remote ahead"
                };

                println!(
                    "  {name}  [{status}] (local v{local_version}, remote v{})",
                    remote_ws.version
                );
            }
            (Some(_), None) => {
                println!("  {name}  [local only]");
            }
            (None, Some(_)) => {
                println!("  {name}  [remote only]");
            }
            (None, None) => {
                // This shouldn't happen
            }
        }
    }

    Ok(())
}

/// Converts a local workspace to API state format.
fn workspace_to_state(ws: &Workspace) -> WorkspaceState {
    WorkspaceState {
        branch: ws.branch.clone(),
        commit_sha: ws.commit_sha.clone(),
        stash_name: ws.stash_name.clone(),
        repo_path: ws.repo_path.to_string_lossy().to_string(),
        metadata: WorkspaceStateMetadata {
            uncommitted_files: ws.metadata.uncommitted_files,
            was_dirty: ws.metadata.was_dirty,
        },
    }
}

/// Converts a remote workspace to local workspace format.
fn state_to_workspace(remote: &RemoteWorkspace) -> Workspace {
    use std::path::PathBuf;

    let mut ws = Workspace::new(
        remote.name.clone(),
        PathBuf::from(&remote.state.repo_path),
        remote.state.branch.clone(),
        remote.state.commit_sha.clone(),
    );

    ws.stash_name = remote.state.stash_name.clone();
    ws.description = remote.description.clone();
    ws.created_at = remote.created_at;
    ws.updated_at = remote.updated_at;
    ws.metadata.uncommitted_files = remote.state.metadata.uncommitted_files;
    ws.metadata.was_dirty = remote.state.metadata.was_dirty;
    ws.metadata.remote_id = Some(remote.id.clone());
    ws.metadata.remote_version = Some(remote.version);
    ws.metadata.last_synced_at = remote.last_synced_at;

    ws
}
