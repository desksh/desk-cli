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
        #[arg(short, long, value_enum, default_value = "git-hub")]
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parse_open_command() {
        let cli = Cli::try_parse_from(["desk", "open", "my-workspace"]).unwrap();
        match cli.command {
            Commands::Open {
                name,
                force,
                interactive,
                ..
            } => {
                assert_eq!(name, Some("my-workspace".to_string()));
                assert!(!force);
                assert!(!interactive);
            },
            _ => panic!("Expected Open command"),
        }
    }

    #[test]
    fn parse_open_with_force() {
        let cli = Cli::try_parse_from(["desk", "open", "ws", "--force"]).unwrap();
        match cli.command {
            Commands::Open { force, .. } => assert!(force),
            _ => panic!("Expected Open command"),
        }
    }

    #[test]
    fn parse_open_interactive() {
        let cli = Cli::try_parse_from(["desk", "open", "--interactive"]).unwrap();
        match cli.command {
            Commands::Open {
                interactive, name, ..
            } => {
                assert!(interactive);
                assert!(name.is_none());
            },
            _ => panic!("Expected Open command"),
        }
    }

    #[test]
    fn parse_list_command() {
        let cli = Cli::try_parse_from(["desk", "list"]).unwrap();
        match cli.command {
            Commands::List { tag, archived, all } => {
                assert!(tag.is_none());
                assert!(!archived);
                assert!(!all);
            },
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn parse_list_with_tag() {
        let cli = Cli::try_parse_from(["desk", "list", "--tag", "feature"]).unwrap();
        match cli.command {
            Commands::List { tag, .. } => {
                assert_eq!(tag, Some("feature".to_string()));
            },
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn parse_list_archived() {
        let cli = Cli::try_parse_from(["desk", "list", "--archived"]).unwrap();
        match cli.command {
            Commands::List { archived, .. } => assert!(archived),
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn parse_status_command() {
        let cli = Cli::try_parse_from(["desk", "status"]).unwrap();
        assert!(matches!(cli.command, Commands::Status));
    }

    #[test]
    fn parse_close_command() {
        let cli = Cli::try_parse_from(["desk", "close"]).unwrap();
        match cli.command {
            Commands::Close { switch_to } => assert!(switch_to.is_none()),
            _ => panic!("Expected Close command"),
        }
    }

    #[test]
    fn parse_close_with_switch() {
        let cli = Cli::try_parse_from(["desk", "close", "--switch-to", "other"]).unwrap();
        match cli.command {
            Commands::Close { switch_to } => {
                assert_eq!(switch_to, Some("other".to_string()));
            },
            _ => panic!("Expected Close command"),
        }
    }

    #[test]
    fn parse_delete_command() {
        let cli = Cli::try_parse_from(["desk", "delete", "ws-name"]).unwrap();
        match cli.command {
            Commands::Delete { name, cloud, yes } => {
                assert_eq!(name, "ws-name");
                assert!(!cloud);
                assert!(!yes);
            },
            _ => panic!("Expected Delete command"),
        }
    }

    #[test]
    fn parse_delete_with_yes() {
        let cli = Cli::try_parse_from(["desk", "delete", "ws", "-y"]).unwrap();
        match cli.command {
            Commands::Delete { yes, .. } => assert!(yes),
            _ => panic!("Expected Delete command"),
        }
    }

    #[test]
    fn parse_rename_command() {
        let cli = Cli::try_parse_from(["desk", "rename", "old", "new"]).unwrap();
        match cli.command {
            Commands::Rename {
                name,
                new_name,
                cloud,
            } => {
                assert_eq!(name, "old");
                assert_eq!(new_name, "new");
                assert!(!cloud);
            },
            _ => panic!("Expected Rename command"),
        }
    }

    #[test]
    fn parse_info_command() {
        let cli = Cli::try_parse_from(["desk", "info", "my-ws"]).unwrap();
        match cli.command {
            Commands::Info { name } => assert_eq!(name, "my-ws"),
            _ => panic!("Expected Info command"),
        }
    }

    #[test]
    fn parse_clone_command() {
        let cli = Cli::try_parse_from(["desk", "clone", "source", "target"]).unwrap();
        match cli.command {
            Commands::Clone { name, new_name } => {
                assert_eq!(name, "source");
                assert_eq!(new_name, "target");
            },
            _ => panic!("Expected Clone command"),
        }
    }

    #[test]
    fn parse_search_command() {
        let cli = Cli::try_parse_from(["desk", "search", "query"]).unwrap();
        match cli.command {
            Commands::Search {
                query,
                name_only,
                branch_only,
            } => {
                assert_eq!(query, "query");
                assert!(!name_only);
                assert!(!branch_only);
            },
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn parse_search_name_only() {
        let cli = Cli::try_parse_from(["desk", "search", "q", "--name-only"]).unwrap();
        match cli.command {
            Commands::Search { name_only, .. } => assert!(name_only),
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn parse_doctor_command() {
        let cli = Cli::try_parse_from(["desk", "doctor"]).unwrap();
        assert!(matches!(cli.command, Commands::Doctor));
    }

    #[test]
    fn parse_history_command() {
        let cli = Cli::try_parse_from(["desk", "history"]).unwrap();
        match cli.command {
            Commands::History { limit, repo_only } => {
                assert_eq!(limit, 10); // default
                assert!(!repo_only);
            },
            _ => panic!("Expected History command"),
        }
    }

    #[test]
    fn parse_history_with_limit() {
        let cli = Cli::try_parse_from(["desk", "history", "--limit", "5"]).unwrap();
        match cli.command {
            Commands::History { limit, .. } => assert_eq!(limit, 5),
            _ => panic!("Expected History command"),
        }
    }

    #[test]
    fn parse_archive_command() {
        let cli = Cli::try_parse_from(["desk", "archive", "ws"]).unwrap();
        match cli.command {
            Commands::Archive { name } => assert_eq!(name, "ws"),
            _ => panic!("Expected Archive command"),
        }
    }

    #[test]
    fn parse_unarchive_command() {
        let cli = Cli::try_parse_from(["desk", "unarchive", "ws"]).unwrap();
        match cli.command {
            Commands::Unarchive { name } => assert_eq!(name, "ws"),
            _ => panic!("Expected Unarchive command"),
        }
    }

    #[test]
    fn parse_diff_command() {
        let cli = Cli::try_parse_from(["desk", "diff", "ws1", "ws2"]).unwrap();
        match cli.command {
            Commands::Diff {
                workspace1,
                workspace2,
            } => {
                assert_eq!(workspace1, "ws1");
                assert_eq!(workspace2, "ws2");
            },
            _ => panic!("Expected Diff command"),
        }
    }

    #[test]
    fn parse_stats_command() {
        let cli = Cli::try_parse_from(["desk", "stats"]).unwrap();
        assert!(matches!(cli.command, Commands::Stats));
    }

    #[test]
    fn parse_watch_command() {
        let cli = Cli::try_parse_from(["desk", "watch"]).unwrap();
        match cli.command {
            Commands::Watch { interval, name } => {
                assert_eq!(interval, 300); // default
                assert!(name.is_none());
            },
            _ => panic!("Expected Watch command"),
        }
    }

    #[test]
    fn parse_watch_with_interval() {
        let cli = Cli::try_parse_from(["desk", "watch", "--interval", "60"]).unwrap();
        match cli.command {
            Commands::Watch { interval, .. } => assert_eq!(interval, 60),
            _ => panic!("Expected Watch command"),
        }
    }

    #[test]
    fn parse_tag_add() {
        let cli = Cli::try_parse_from(["desk", "tag", "ws", "add", "tag1", "tag2"]).unwrap();
        match cli.command {
            Commands::Tag {
                name,
                command: TagCommands::Add { tags },
            } => {
                assert_eq!(name, "ws");
                assert_eq!(tags, vec!["tag1", "tag2"]);
            },
            _ => panic!("Expected Tag Add command"),
        }
    }

    #[test]
    fn parse_tag_remove() {
        let cli = Cli::try_parse_from(["desk", "tag", "ws", "remove", "tag1"]).unwrap();
        match cli.command {
            Commands::Tag {
                command: TagCommands::Remove { tags },
                ..
            } => {
                assert_eq!(tags, vec!["tag1"]);
            },
            _ => panic!("Expected Tag Remove command"),
        }
    }

    #[test]
    fn parse_tag_list() {
        let cli = Cli::try_parse_from(["desk", "tag", "ws", "list"]).unwrap();
        match cli.command {
            Commands::Tag {
                command: TagCommands::List,
                ..
            } => {},
            _ => panic!("Expected Tag List command"),
        }
    }

    #[test]
    fn parse_alias_set() {
        let cli = Cli::try_parse_from(["desk", "alias", "set", "f", "feature"]).unwrap();
        match cli.command {
            Commands::Alias {
                command: AliasCommands::Set { alias, workspace },
            } => {
                assert_eq!(alias, "f");
                assert_eq!(workspace, "feature");
            },
            _ => panic!("Expected Alias Set command"),
        }
    }

    #[test]
    fn parse_alias_remove() {
        let cli = Cli::try_parse_from(["desk", "alias", "remove", "f"]).unwrap();
        match cli.command {
            Commands::Alias {
                command: AliasCommands::Remove { alias },
            } => {
                assert_eq!(alias, "f");
            },
            _ => panic!("Expected Alias Remove command"),
        }
    }

    #[test]
    fn parse_hooks_add() {
        let cli =
            Cli::try_parse_from(["desk", "hooks", "add", "pre-switch", "echo hello"]).unwrap();
        match cli.command {
            Commands::Hooks {
                command: HookCommands::Add { hook_type, command },
            } => {
                assert!(matches!(hook_type, HookType::PreSwitch));
                assert_eq!(command, "echo hello");
            },
            _ => panic!("Expected Hooks Add command"),
        }
    }

    #[test]
    fn parse_note_add() {
        let cli = Cli::try_parse_from(["desk", "note", "ws", "add", "my note"]).unwrap();
        match cli.command {
            Commands::Note {
                name,
                command: NoteCommands::Add { text },
            } => {
                assert_eq!(name, "ws");
                assert_eq!(text, "my note");
            },
            _ => panic!("Expected Note Add command"),
        }
    }

    #[test]
    fn parse_bulk_delete() {
        let cli = Cli::try_parse_from(["desk", "bulk", "delete", "ws1", "ws2", "-y"]).unwrap();
        match cli.command {
            Commands::Bulk {
                command: BulkCommands::Delete { names, yes },
            } => {
                assert_eq!(names, vec!["ws1", "ws2"]);
                assert!(yes);
            },
            _ => panic!("Expected Bulk Delete command"),
        }
    }

    #[test]
    fn parse_sync_push() {
        let cli = Cli::try_parse_from(["desk", "sync", "push"]).unwrap();
        match cli.command {
            Commands::Sync {
                command: SyncCommands::Push { name, force },
            } => {
                assert!(name.is_none());
                assert!(!force);
            },
            _ => panic!("Expected Sync Push command"),
        }
    }

    #[test]
    fn parse_sync_pull_with_force() {
        let cli = Cli::try_parse_from(["desk", "sync", "pull", "--force"]).unwrap();
        match cli.command {
            Commands::Sync {
                command: SyncCommands::Pull { force, .. },
            } => {
                assert!(force);
            },
            _ => panic!("Expected Sync Pull command"),
        }
    }

    #[test]
    fn parse_auth_login() {
        let cli = Cli::try_parse_from(["desk", "auth", "login"]).unwrap();
        match cli.command {
            Commands::Auth {
                command:
                    AuthCommands::Login {
                        provider,
                        no_browser,
                    },
            } => {
                assert!(matches!(provider, ProviderArg::GitHub)); // default
                assert!(!no_browser);
            },
            _ => panic!("Expected Auth Login command"),
        }
    }

    #[test]
    fn parse_auth_login_google() {
        // Note: clap converts PascalCase to kebab-case for value enum
        let cli = Cli::try_parse_from(["desk", "auth", "login", "-p", "google"]).unwrap();
        match cli.command {
            Commands::Auth {
                command: AuthCommands::Login { provider, .. },
            } => {
                assert!(matches!(provider, ProviderArg::Google));
            },
            _ => panic!("Expected Auth Login command"),
        }
    }

    #[test]
    fn parse_auth_logout() {
        let cli = Cli::try_parse_from(["desk", "auth", "logout"]).unwrap();
        match cli.command {
            Commands::Auth {
                command: AuthCommands::Logout,
            } => {},
            _ => panic!("Expected Auth Logout command"),
        }
    }

    #[test]
    fn parse_auth_status() {
        let cli = Cli::try_parse_from(["desk", "auth", "status"]).unwrap();
        match cli.command {
            Commands::Auth {
                command: AuthCommands::Status,
            } => {},
            _ => panic!("Expected Auth Status command"),
        }
    }

    #[test]
    fn parse_init_bash() {
        let cli = Cli::try_parse_from(["desk", "init", "bash"]).unwrap();
        match cli.command {
            Commands::Init { shell } => {
                assert!(matches!(shell, ShellType::Bash));
            },
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn parse_completions_zsh() {
        let cli = Cli::try_parse_from(["desk", "completions", "zsh"]).unwrap();
        match cli.command {
            Commands::Completions { shell } => {
                assert!(matches!(shell, ShellType::Zsh));
            },
            _ => panic!("Expected Completions command"),
        }
    }

    #[test]
    fn provider_arg_to_auth_provider() {
        assert!(matches!(
            AuthProvider::from(ProviderArg::GitHub),
            AuthProvider::GitHub
        ));
        assert!(matches!(
            AuthProvider::from(ProviderArg::Google),
            AuthProvider::Google
        ));
    }

    #[test]
    fn global_verbose_flag() {
        let cli = Cli::try_parse_from(["desk", "--verbose", "status"]).unwrap();
        assert!(cli.verbose);
    }
}
