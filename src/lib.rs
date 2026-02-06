//! Library entry point re-exporting core modules for tests and future UI code.

pub mod app;
pub mod config;
pub mod gh_status;
pub mod git_status;
pub mod state;
pub mod tab;
pub mod terminal;
pub mod ui;
pub mod workspace;
pub mod worktree;

pub use app::{App, FocusArea, InputMode};
pub use config::Config;
pub use gh_status::{GhCiStatus, GhWorkspaceStatus};
pub use git_status::GitWorkspaceStatus;
pub use state::{AppState, TabState, WorkspaceState};
pub use workspace::Workspace;
pub use worktree::{WorktreeError, WorktreeInfo, WorktreeManager};

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::{Mutex, MutexGuard};

    static HOME_ENV_LOCK: Mutex<()> = Mutex::new(());

    pub(crate) fn lock_home_env() -> MutexGuard<'static, ()> {
        HOME_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
