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
    // Future commands will be added here:
    // Open, Switch, Close, List, Status, Config
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
