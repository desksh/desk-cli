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
        /// If omitted with --interactive, shows a selection menu.
        name: Option<String>,

        /// Optional description for a new workspace.
        #[arg(short, long)]
        description: Option<String>,

        /// Force overwrite if workspace already exists.
        #[arg(short, long)]
        force: bool,

        /// Interactive mode: select workspace from a list.
        #[arg(short, long)]
        interactive: bool,
    },

    /// List all saved workspaces.
    List {
        /// Filter by tag.
        #[arg(short, long)]
        tag: Option<String>,

        /// Only show archived workspaces.
        #[arg(long)]
        archived: bool,

        /// Show all workspaces including archived.
        #[arg(long)]
        all: bool,
    },

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

    /// Show detailed information about a workspace.
    ///
    /// Displays all workspace metadata including git state, sync status,
    /// and timestamps.
    Info {
        /// Name of the workspace to inspect.
        name: String,
    },

    /// Clone/duplicate a workspace.
    ///
    /// Creates a copy of an existing workspace with a new name.
    Clone {
        /// Name of the workspace to clone.
        name: String,

        /// Name for the new workspace.
        new_name: String,
    },

    /// Update the description of a workspace.
    Describe {
        /// Name of the workspace.
        name: String,

        /// New description for the workspace.
        description: String,

        /// Also update on the cloud (if synced).
        #[arg(long)]
        cloud: bool,
    },

    /// Export a workspace to a JSON file.
    ///
    /// Creates a portable backup of workspace metadata.
    Export {
        /// Name of the workspace to export.
        name: String,

        /// Output file path (defaults to <name>.json).
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Import a workspace from a JSON file.
    ///
    /// Restores workspace metadata from a backup file.
    Import {
        /// Path to the JSON file to import.
        file: String,

        /// Override the workspace name.
        #[arg(short, long)]
        name: Option<String>,

        /// Overwrite if workspace already exists.
        #[arg(short, long)]
        force: bool,
    },

    /// Clean up orphaned desk stashes.
    ///
    /// Removes git stashes created by desk that are no longer referenced
    /// by any workspace.
    Clean {
        /// Actually delete stashes (dry-run by default).
        #[arg(long)]
        execute: bool,
    },

    /// Output current workspace name for shell prompts.
    ///
    /// Returns the name of the currently active workspace, or nothing if
    /// no workspace is active. Designed for use in shell prompt scripts.
    Prompt,

    /// Generate shell integration script.
    ///
    /// Outputs a script to integrate desk with your shell prompt.
    /// Source the output in your shell's rc file.
    Init {
        /// Shell to generate script for.
        #[arg(value_enum)]
        shell: ShellType,
    },

    /// Search workspaces by name, branch, or description.
    Search {
        /// Search query (matches name, branch, or description).
        query: String,

        /// Only search in workspace names.
        #[arg(long)]
        name_only: bool,

        /// Only search in branch names.
        #[arg(long)]
        branch_only: bool,
    },

    /// Generate shell completion scripts.
    ///
    /// Outputs completion script for the specified shell.
    /// Follow shell-specific instructions to install.
    Completions {
        /// Shell to generate completions for.
        #[arg(value_enum)]
        shell: ShellType,
    },

    /// Check desk installation and diagnose issues.
    ///
    /// Verifies configuration, storage, git access, and API connectivity.
    Doctor,

    /// Show workspace switch history.
    History {
        /// Number of entries to show.
        #[arg(short, long, default_value = "10")]
        limit: usize,

        /// Only show history for current repository.
        #[arg(long)]
        repo_only: bool,
    },

    /// View or modify configuration.
    Config {
        /// Configuration key to get or set.
        key: Option<String>,

        /// Value to set (if omitted, shows current value).
        value: Option<String>,

        /// List all configuration values.
        #[arg(short, long)]
        list: bool,
    },

    /// Manage workspace tags.
    Tag {
        /// Workspace name.
        name: String,

        #[command(subcommand)]
        command: TagCommands,
    },

    /// Archive a workspace.
    ///
    /// Hides the workspace from the default list without deleting it.
    Archive {
        /// Name of the workspace to archive.
        name: String,
    },

    /// Unarchive a workspace.
    ///
    /// Restores an archived workspace to the active list.
    Unarchive {
        /// Name of the workspace to unarchive.
        name: String,
    },

    /// Create an alias for a workspace.
    ///
    /// Aliases allow quick access with shorter names.
    Alias {
        #[command(subcommand)]
        command: AliasCommands,
    },

    /// Compare two workspaces.
    ///
    /// Shows differences in git state between two workspaces.
    Diff {
        /// First workspace name.
        workspace1: String,

        /// Second workspace name.
        workspace2: String,
    },

    /// Show workspace usage statistics.
    ///
    /// Displays metrics like most-used workspaces, switch frequency, etc.
    Stats,

    /// Manage workspace hooks.
    ///
    /// Hooks run commands before or after workspace switches.
    Hooks {
        #[command(subcommand)]
        command: HookCommands,
    },

    /// Watch mode: auto-save workspace state periodically.
    Watch {
        /// Interval in seconds between saves.
        #[arg(short, long, default_value = "300")]
        interval: u64,

        /// Workspace name to watch.
        name: Option<String>,
    },

    /// Manage workspace notes.
    Note {
        /// Workspace name.
        name: String,

        #[command(subcommand)]
        command: NoteCommands,
    },

    /// Bulk operations on multiple workspaces.
    Bulk {
        #[command(subcommand)]
        command: BulkCommands,
    },
}

/// Tag subcommands.
#[derive(Subcommand, Debug)]
pub enum TagCommands {
    /// Add tags to a workspace.
    Add {
        /// Tags to add.
        #[arg(required = true)]
        tags: Vec<String>,
    },

    /// Remove tags from a workspace.
    Remove {
        /// Tags to remove.
        #[arg(required = true)]
        tags: Vec<String>,
    },

    /// List tags on a workspace.
    List,

    /// Clear all tags from a workspace.
    Clear,
}

/// Alias subcommands.
#[derive(Subcommand, Debug)]
pub enum AliasCommands {
    /// Create an alias for a workspace.
    Set {
        /// Short alias name.
        alias: String,

        /// Full workspace name.
        workspace: String,
    },

    /// Remove an alias.
    Remove {
        /// Alias to remove.
        alias: String,
    },

    /// List all aliases.
    List,
}

/// Hook subcommands.
#[derive(Subcommand, Debug)]
pub enum HookCommands {
    /// Add a hook.
    Add {
        /// Hook type (pre-switch or post-switch).
        #[arg(value_enum)]
        hook_type: HookType,

        /// Command to run.
        command: String,
    },

    /// Remove a hook.
    Remove {
        /// Hook type.
        #[arg(value_enum)]
        hook_type: HookType,

        /// Index of the hook to remove (0-based).
        index: usize,
    },

    /// List all hooks.
    List,

    /// Clear all hooks.
    Clear,
}

/// Hook types.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum HookType {
    PreSwitch,
    PostSwitch,
}

/// Note subcommands.
#[derive(Subcommand, Debug)]
pub enum NoteCommands {
    /// Add a note to a workspace.
    Add {
        /// Note text.
        text: String,
    },

    /// List notes on a workspace.
    List,

    /// Clear all notes from a workspace.
    Clear,
}

/// Bulk operation subcommands.
#[derive(Subcommand, Debug)]
pub enum BulkCommands {
    /// Delete multiple workspaces.
    Delete {
        /// Workspace names to delete.
        #[arg(required = true)]
        names: Vec<String>,

        /// Skip confirmation prompt.
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Add tags to multiple workspaces.
    Tag {
        /// Workspace names.
        #[arg(required = true, num_args = 1..)]
        names: Vec<String>,

        /// Tags to add.
        #[arg(short, long, required = true, num_args = 1..)]
        tags: Vec<String>,
    },

    /// Archive multiple workspaces.
    Archive {
        /// Workspace names to archive.
        #[arg(required = true)]
        names: Vec<String>,
    },

    /// Export multiple workspaces.
    Export {
        /// Workspace names to export.
        #[arg(required = true)]
        names: Vec<String>,

        /// Output directory.
        #[arg(short, long, default_value = ".")]
        output: String,
    },
}

/// Supported shell types for init command.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
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
