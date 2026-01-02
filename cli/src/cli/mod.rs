//! CLI module for desk.

pub mod args;
pub mod commands;

pub use args::{
    AliasCommands, AuthCommands, BulkCommands, Cli, Commands, HookCommands, HookType,
    NoteCommands, SyncCommands, TagCommands,
};
