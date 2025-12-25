//! Workspace management for desk-cli.
//!
//! This module provides workspace state persistence:
//! - Save current git context as a named workspace
//! - Load and restore workspace state
//! - List and delete saved workspaces

pub mod error;
pub mod storage;
pub mod types;

#[allow(unused_imports)]
pub use error::WorkspaceError;
#[allow(unused_imports)]
pub use storage::{FileWorkspaceStore, WorkspaceStore};
#[allow(unused_imports)]
pub use types::{Workspace, WorkspaceMetadata};
