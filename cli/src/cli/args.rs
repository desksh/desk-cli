//! Command-line argument parsing.

use clap::{Parser, Subcommand, ValueEnum};

use crate::auth::AuthProvider;

/// Developer context switching tool.
///
/// Desk eliminates the hidden tax of context switching by capturing and
/// restoring complete development contexts—git state, open files, running
/// services, and more.
#[derive(Parser, Debug)]
#[command(name = "desk")]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose output.
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

/// Available commands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Manage authentication.
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },

    /// Create or restore a workspace.
    ///
    /// If the workspace exists, restores its git state (branch, stashed changes).
    /// If it doesn't exist, saves the current git state as a new workspace.
    Open {
        /// Name of the workspace to open or create.
        name: String,

        /// Optional description for a new workspace.
        #[arg(short, long)]
        description: Option<String>,

        /// Force overwrite if workspace already exists.
        #[arg(short, long)]
        force: bool,
    },

    /// List all saved workspaces.
    List,

    /// Show current workspace and git status.
    Status,

    /// Close the current workspace.
    ///
    /// Saves the current state and optionally switches to another workspace.
    Close {
        /// Workspace to switch to after closing.
        #[arg(short, long)]
        switch_to: Option<String>,
    },

    /// Sync workspaces with the cloud (Pro feature).
    ///
    /// Push and pull workspaces to sync across devices.
    Sync {
        #[command(subcommand)]
        command: SyncCommands,
    },

    /// Delete a workspace.
    ///
    /// Removes the workspace locally and optionally from the cloud.
    Delete {
        /// Name of the workspace to delete.
        name: String,

        /// Also delete from the cloud (if synced).
        #[arg(long)]
        cloud: bool,

        /// Skip confirmation prompt.
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Rename a workspace.
    ///
    /// Changes the workspace name locally and optionally on the cloud.
    Rename {
        /// Current name of the workspace.
        name: String,

        /// New name for the workspace.
        new_name: String,

        /// Also rename on the cloud (if synced).
        #[arg(long)]
        cloud: bool,
    },
}

/// Sync subcommands.
#[derive(Subcommand, Debug)]
pub enum SyncCommands {
    /// Push local workspaces to the cloud.
    ///
    /// Uploads workspace state to sync across devices.
    Push {
        /// Workspace name to push (or all if omitted).
        name: Option<String>,

        /// Force push even if remote has newer changes.
        #[arg(short, long)]
        force: bool,
    },

    /// Pull workspaces from the cloud.
    ///
    /// Downloads workspace state from the cloud.
    Pull {
        /// Workspace name to pull (or all if omitted).
        name: Option<String>,

        /// Force pull even if local has newer changes.
        #[arg(short, long)]
        force: bool,
    },

    /// Show sync status.
    ///
    /// Displays which workspaces are synced, out of sync, or local-only.
    Status,
}

/// Authentication subcommands.
#[derive(Subcommand, Debug)]
pub enum AuthCommands {
    /// Log in to Desk using OAuth.
    Login {
        /// OAuth provider to use.
        #[arg(short, long, value_enum, default_value = "github")]
        provider: ProviderArg,

        /// Skip opening the browser automatically.
        #[arg(long)]
        no_browser: bool,
    },

    /// Log out and remove stored credentials.
    Logout,

    /// Show current authentication status.
    Status,
}

/// Provider argument for CLI.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ProviderArg {
    GitHub,
    Google,
}

impl From<ProviderArg> for AuthProvider {
    fn from(arg: ProviderArg) -> Self {
        match arg {
            ProviderArg::GitHub => Self::GitHub,
            ProviderArg::Google => Self::Google,
        }
    }
}
