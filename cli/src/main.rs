//! Desk CLI - Developer Context Switching Tool
//!
//! Desk eliminates the hidden tax of context switching by capturing and
//! restoring complete development contexts—git state, open files, running
//! services, and more.

mod auth;
mod cli;
mod client;
mod config;
mod error;
mod git;
mod workspace;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::cli::{AuthCommands, Cli, Commands, SyncCommands};
use crate::error::Result;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("DESK_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Run the command
    if let Err(e) = run(cli).await {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Auth { command } => match command {
            AuthCommands::Login {
                provider,
                no_browser,
            } => cli::commands::handle_login(provider.into(), no_browser).await,
            AuthCommands::Logout => cli::commands::handle_logout().await,
            AuthCommands::Status => cli::commands::handle_status().await,
        },
        Commands::Open {
            name,
            description,
            force,
        } => cli::commands::handle_open(&name, description, force),
        Commands::List => cli::commands::handle_list(),
        Commands::Status => cli::commands::handle_workspace_status(),
        Commands::Close { switch_to } => cli::commands::handle_close(switch_to),
        Commands::Sync { command } => match command {
            SyncCommands::Push { name, force } => {
                cli::commands::handle_sync_push(name, force).await
            }
            SyncCommands::Pull { name, force } => {
                cli::commands::handle_sync_pull(name, force).await
            }
            SyncCommands::Status => cli::commands::handle_sync_status().await,
        },
    }
}
