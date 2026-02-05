//! Library entry point re-exporting core modules for tests and future UI code.

pub mod app;
pub mod config;
pub mod state;
pub mod ui;
pub mod workspace;

pub use app::{App, FocusArea};
pub use config::Config;
pub use state::{AppState, WorkspaceState};
pub use workspace::Workspace;
