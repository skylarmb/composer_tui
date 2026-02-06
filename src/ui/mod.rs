//! UI rendering module.
//!
//! Provides the main render function and submodules for each UI component.

mod header;
mod main_panel;
mod sidebar;

use ratatui::{
    layout::{Constraint, Layout, Rect},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::{App, FocusArea, InputMode};

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
    let focus = app.focus();
    header::render(frame, chunks[0], focus == FocusArea::Header);

    // Horizontal split: Sidebar | Main Panel
    let body_chunks = Layout::horizontal([
        Constraint::Length(12), // Sidebar (fixed width)
        Constraint::Min(0),     // Main panel (remaining space)
    ])
    .split(chunks[1]);

    // Render sidebar and main panel
    sidebar::render(frame, body_chunks[0], app, focus == FocusArea::Sidebar);
    main_panel::render(frame, body_chunks[1], app, focus == FocusArea::Main);
    render_modal(frame, app);
}

fn render_modal(frame: &mut Frame, app: &App) {
    let (title, body) = match app.input_mode() {
        InputMode::Normal => return,
        InputMode::CreateWorkspace { name } => (
            "Create Workspace",
            format!("Name: {name}\n\nEnter = create\nEsc = cancel"),
        ),
        InputMode::ConfirmDelete { workspace_name } => (
            "Delete Workspace",
            format!(
                "Delete workspace '{workspace_name}'?\n\nThis removes the git worktree.\n\nEnter = confirm\nEsc = cancel"
            ),
        ),
        InputMode::Error { message } => (
            "Error",
            format!("{message}\n\nEnter or Esc to dismiss"),
        ),
    };

    let area = centered_rect(60, 40, frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(body).block(Block::default().title(title).borders(Borders::ALL)),
        area,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    let horizontal = Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1]);

    horizontal[1]
}
