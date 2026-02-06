//! UI rendering module.
//!
//! Provides the main render function and submodules for each UI component.

mod header;
mod main_panel;
mod sidebar;
mod status_bar;

use ratatui::{
    layout::{Constraint, Layout, Rect},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::{App, FocusArea, InputMode};

// Temporarily disable the top bar while keeping header code available.
const SHOW_TOP_BAR: bool = false;
const TOP_BAR_HEIGHT: u16 = if SHOW_TOP_BAR { 3 } else { 0 };

/// Render the entire UI layout.
///
/// Layout structure:
/// - Vertical split: Header (optional) | Body | StatusBar (1 row)
/// - Horizontal split of Body: Sidebar (configurable width) | MainPanel
/// - When fullscreen: sidebar is hidden, main panel takes full body width
pub fn render(frame: &mut Frame, app: &App) {
    let sidebar_width = app.config().sidebar_width();
    let focused_border_color = app.config().focused_border_color();

    // Vertical split: Header | Body | StatusBar
    let chunks = Layout::vertical([
        Constraint::Length(TOP_BAR_HEIGHT), // Header (disabled for now)
        Constraint::Min(0),                 // Body (remaining space)
        Constraint::Length(1),              // Status bar (single row, no border)
    ])
    .split(frame.area());

    let focus = app.focus();
    if SHOW_TOP_BAR {
        header::render(
            frame,
            chunks[0],
            focus == FocusArea::Header,
            focused_border_color,
        );
    }

    if app.is_fullscreen() {
        // Fullscreen: main panel takes entire body width (no sidebar).
        main_panel::render(
            frame,
            chunks[1],
            app,
            focus == FocusArea::Main,
            focused_border_color,
        );
    } else {
        // Normal: Sidebar | Main Panel
        let body_chunks = Layout::horizontal([
            Constraint::Length(sidebar_width), // Sidebar (configurable width)
            Constraint::Min(0),                // Main panel (remaining space)
        ])
        .split(chunks[1]);

        sidebar::render(
            frame,
            body_chunks[0],
            app,
            focus == FocusArea::Sidebar,
            focused_border_color,
        );
        main_panel::render(
            frame,
            body_chunks[1],
            app,
            focus == FocusArea::Main,
            focused_border_color,
        );
    }

    status_bar::render(frame, chunks[2], app);
    render_modal(frame, app);
}

/// Compute PTY dimensions for the main panel content area.
///
/// When `fullscreen` is true, the sidebar is hidden and the main panel
/// occupies the entire body width.
pub fn main_panel_terminal_size(
    width: u16,
    height: u16,
    fullscreen: bool,
    sidebar_width: u16,
) -> (u16, u16) {
    let frame_area = Rect::new(0, 0, width, height);
    let chunks = Layout::vertical([
        Constraint::Length(TOP_BAR_HEIGHT),
        Constraint::Min(0),
        Constraint::Length(1), // Status bar
    ])
    .split(frame_area);

    let main = if fullscreen {
        // No sidebar — main panel gets the entire body.
        chunks[1]
    } else {
        let body_chunks =
            Layout::horizontal([Constraint::Length(sidebar_width), Constraint::Min(0)])
                .split(chunks[1]);
        body_chunks[1]
    };

    (
        main.width.saturating_sub(2).max(1),
        main.height.saturating_sub(2).max(1),
    )
}

/// Layout regions used for mouse hit-testing.
///
/// Returns `(header_rect, sidebar_rect, main_rect, status_bar_rect)`.
/// When `fullscreen` is true, `sidebar_rect` is `None`.
pub fn layout_rects(
    width: u16,
    height: u16,
    fullscreen: bool,
    sidebar_width: u16,
) -> (Rect, Option<Rect>, Rect, Rect) {
    let frame_area = Rect::new(0, 0, width, height);
    let chunks = Layout::vertical([
        Constraint::Length(TOP_BAR_HEIGHT),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(frame_area);

    let header = chunks[0];
    let status_bar = chunks[2];

    if fullscreen {
        (header, None, chunks[1], status_bar)
    } else {
        let body_chunks =
            Layout::horizontal([Constraint::Length(sidebar_width), Constraint::Min(0)])
                .split(chunks[1]);
        (header, Some(body_chunks[0]), body_chunks[1], status_bar)
    }
}

/// Resolve a clicked tab index from main panel top border coordinates.
pub fn main_panel_tab_index_at(main_rect: Rect, app: &App, col: u16, row: u16) -> Option<usize> {
    main_panel::tab_index_at(main_rect, app, col, row)
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
        InputMode::ConfirmCloseTab => (
            "Close Running Tab",
            "The active tab still has a running process.\n\nClose it anyway?\n\nEnter = close tab\nEsc = cancel".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_panel_terminal_size_accounts_for_status_bar() {
        // 80x24 terminal, non-fullscreen, sidebar_width=20:
        // Body height = 24 - 0 (header disabled) - 1 (status bar) = 23
        // Main width = 80 - 20 (sidebar) = 60, inner = 60 - 2 = 58
        // Main height inner = 23 - 2 = 21
        let (cols, rows) = main_panel_terminal_size(80, 24, false, 20);
        assert_eq!(cols, 58);
        assert_eq!(rows, 21);
    }

    #[test]
    fn main_panel_terminal_size_fullscreen_uses_full_width() {
        // Fullscreen: no sidebar, sidebar_width ignored
        // Main width = 80, inner = 80 - 2 = 78
        // Height same as above = 21
        let (cols, rows) = main_panel_terminal_size(80, 24, true, 20);
        assert_eq!(cols, 78);
        assert_eq!(rows, 21);
    }

    #[test]
    fn layout_rects_fullscreen_has_no_sidebar() {
        let (_, sidebar, _, _) = layout_rects(80, 24, true, 20);
        assert!(sidebar.is_none());
    }

    #[test]
    fn layout_rects_normal_has_sidebar() {
        let (_, sidebar, _, _) = layout_rects(80, 24, false, 20);
        assert!(sidebar.is_some());
        assert_eq!(sidebar.unwrap().width, 20);
    }

    #[test]
    fn sidebar_width_is_configurable() {
        let (_, sidebar, _, _) = layout_rects(80, 24, false, 30);
        assert_eq!(sidebar.unwrap().width, 30);

        let (cols, _) = main_panel_terminal_size(80, 24, false, 30);
        // 80 - 30 = 50, inner = 50 - 2 = 48
        assert_eq!(cols, 48);
    }
}
