//! Library entry point re-exporting core modules for tests and future UI code.

pub mod app;
pub mod ui;
pub mod workspace;

pub use app::{App, FocusArea};
pub use workspace::Workspace;
