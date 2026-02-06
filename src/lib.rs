//! Library entry point re-exporting core modules for tests and future UI code.

pub mod app;
pub mod config;
pub mod state;
pub mod terminal;
pub mod ui;
pub mod workspace;
pub mod worktree;

pub use app::{App, FocusArea, InputMode};
pub use config::Config;
pub use state::{AppState, WorkspaceState};
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
