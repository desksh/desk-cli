//! Command implementations.

pub mod auth;
pub mod sync;
pub mod workspace;

pub use auth::{handle_login, handle_logout, handle_status};
pub use sync::{handle_sync_pull, handle_sync_push, handle_sync_status};
pub use workspace::{
    handle_alias, handle_archive, handle_bulk, handle_clean, handle_clone, handle_close,
    handle_completions, handle_config, handle_delete, handle_describe, handle_diff, handle_doctor,
    handle_export, handle_history, handle_hooks, handle_import, handle_info, handle_init,
    handle_list, handle_note, handle_open, handle_prompt, handle_rename, handle_search,
    handle_stats, handle_tag, handle_unarchive, handle_watch, handle_workspace_status,
};
