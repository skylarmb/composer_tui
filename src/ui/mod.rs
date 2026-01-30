//! UI rendering module.
//!
//! Provides the main render function and submodules for each UI component.

mod header;
mod main_panel;
mod sidebar;

use ratatui::{
    layout::{Constraint, Layout},
    Frame,
};

use crate::App;

/// Render the entire UI layout.
///
/// Layout structure:
/// - Vertical split: Header (3 rows) | Body
/// - Horizontal split of Body: Sidebar (12 cols) | MainPanel
pub fn render(frame: &mut Frame, app: &App) {
    // Vertical split: Header | Body
    let chunks = Layout::vertical([
        Constraint::Length(3), // Header (1 row + 2 for borders)
        Constraint::Min(0),    // Body (remaining space)
    ])
    .split(frame.area());

    // Render header
    header::render(frame, chunks[0]);

    // Horizontal split: Sidebar | Main Panel
    let body_chunks = Layout::horizontal([
        Constraint::Length(12), // Sidebar (fixed width)
        Constraint::Min(0),     // Main panel (remaining space)
    ])
    .split(chunks[1]);

    // Render sidebar and main panel
    sidebar::render(frame, body_chunks[0], app);
    main_panel::render(frame, body_chunks[1]);
}
