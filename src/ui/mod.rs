//! UI rendering module.
//!
//! Provides the main render function and submodules for each UI component.

mod header;
mod main_panel;
mod sidebar;
mod status_bar;

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::{App, FocusArea, InputMode};

const TOP_BAR_HEIGHT: u16 = 3;

/// Render the entire UI layout.
///
/// Layout structure:
/// - Vertical split: Header | Body | StatusBar (1 row)
/// - Horizontal split of Body: Sidebar (configurable width) | MainPanel
/// - When fullscreen: sidebar is hidden, main panel takes full body width
/// - When zen mode: all chrome is hidden, main panel takes the entire frame
pub fn render(frame: &mut Frame, app: &App) {
    let sidebar_width = app.config().sidebar_width();
    let focused_border_color = app.config().focused_border_color();

    if app.is_zen_mode() {
        // Zen mode: main panel fills the entire frame with no chrome.
        let focus = app.focus();
        main_panel::render(
            frame,
            frame.area(),
            app,
            focus == FocusArea::Main,
            focused_border_color,
            true,
        );
        render_modal(frame, app);
        return;
    }

    // Vertical split: Header | Body | StatusBar
    let chunks = Layout::vertical([
        Constraint::Length(TOP_BAR_HEIGHT), // Header
        Constraint::Min(0),                 // Body (remaining space)
        Constraint::Length(1),              // Status bar (single row, no border)
    ])
    .split(frame.area());

    let focus = app.focus();
    header::render(
        frame,
        chunks[0],
        app,
        focus == FocusArea::Header,
        focused_border_color,
    );

    if app.is_fullscreen() {
        // Fullscreen: main panel takes entire body width (no sidebar).
        main_panel::render(
            frame,
            chunks[1],
            app,
            focus == FocusArea::Main,
            focused_border_color,
            false,
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
            false,
        );
    }

    status_bar::render(frame, chunks[2], app);
    render_modal(frame, app);
}

/// Compute PTY dimensions for the main panel content area.
///
/// When `zen_mode` is true, all chrome is hidden and the terminal occupies
/// the entire frame (no borders subtracted).
/// When `fullscreen` is true, the sidebar is hidden and the main panel
/// occupies the entire body width.
pub fn main_panel_terminal_size(
    width: u16,
    height: u16,
    fullscreen: bool,
    sidebar_width: u16,
    zen_mode: bool,
) -> (u16, u16) {
    if zen_mode {
        // Zen mode: terminal fills the full frame with no borders.
        return (width.max(1), height.max(1));
    }

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
/// When `zen_mode` is true, the main panel occupies the full frame and
/// `sidebar_rect` is `None`.
pub fn layout_rects(
    width: u16,
    height: u16,
    fullscreen: bool,
    sidebar_width: u16,
    zen_mode: bool,
) -> (Rect, Option<Rect>, Rect, Rect) {
    let frame_area = Rect::new(0, 0, width, height);

    if zen_mode {
        // Zen mode: main panel is the entire frame; header and status bar are empty.
        let empty = Rect::new(0, 0, 0, 0);
        return (empty, None, frame_area, empty);
    }

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
    match app.input_mode() {
        InputMode::Normal => {}
        InputMode::ChangesPanel { lines } => {
            let body = lines.join("\n") + "\n\ng/Esc = close · C = commit & push";
            let area = centered_rect(70, 80, frame.area());
            frame.render_widget(Clear, area);
            frame.render_widget(
                Paragraph::new(body).block(Block::default().title("Changes").borders(Borders::ALL)),
                area,
            );
        }
        InputMode::DiffViewer {
            lines,
            scroll,
            show_branch_diff,
        } => {
            let diff_kind = if *show_branch_diff {
                "branch (git diff HEAD)"
            } else {
                "unstaged (git diff)"
            };
            let title = format!("Diff — {diff_kind}");
            let area = centered_rect(92, 90, frame.area());
            frame.render_widget(Clear, area);

            // Height available for lines (subtract 2 for the border).
            let visible = area.height.saturating_sub(2) as usize;
            let styled_lines: Vec<Line> = lines
                .iter()
                .skip(*scroll)
                .take(visible)
                .map(|l| diff_line_to_styled(l))
                .collect();

            frame.render_widget(
                Paragraph::new(styled_lines)
                    .block(Block::default().title(title).borders(Borders::ALL)),
                area,
            );
        }
        mode => {
            let (title, body) = match mode {
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
                    "The active tab still has a running process.\n\nClose it anyway?\n\nEnter = close tab\nEsc = cancel"
                        .to_string(),
                ),
                InputMode::CommitMessage { message } => (
                    "Commit & Push",
                    format!("Message: {message}\n\nEnter = commit & push\nEsc = cancel"),
                ),
                InputMode::Error { message } => (
                    "Error",
                    format!("{message}\n\nEnter or Esc to dismiss"),
                ),
                InputMode::Normal | InputMode::ChangesPanel { .. } | InputMode::DiffViewer { .. } => {
                    return
                }
            };

            let area = centered_rect(60, 40, frame.area());
            frame.render_widget(Clear, area);
            frame.render_widget(
                Paragraph::new(body).block(Block::default().title(title).borders(Borders::ALL)),
                area,
            );
        }
    }
}

/// Apply syntax-highlighting to a single diff patch line.
///
/// Colours follow the standard diff convention:
/// - Added lines (`+`)  → green
/// - Removed lines (`-`) → red
/// - Hunk headers (`@@`) → cyan
/// - File headers (`diff `, `index `, `---`, `+++`) → yellow
/// - Context lines → default foreground
fn diff_line_to_styled(line: &str) -> Line<'static> {
    let style = if line.starts_with("+++")
        || line.starts_with("---")
        || line.starts_with("diff ")
        || line.starts_with("index ")
    {
        Style::default().fg(Color::Yellow)
    } else if line.starts_with('+') {
        Style::default().fg(Color::Green)
    } else if line.starts_with('-') {
        Style::default().fg(Color::Red)
    } else if line.starts_with("@@") {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    Line::from(Span::styled(line.to_string(), style))
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
        // Body height = 24 - 3 (header) - 1 (status bar) = 20
        // Main width = 80 - 20 (sidebar) = 60, inner = 60 - 2 = 58
        // Main height inner = 20 - 2 = 18
        let (cols, rows) = main_panel_terminal_size(80, 24, false, 20, false);
        assert_eq!(cols, 58);
        assert_eq!(rows, 18);
    }

    #[test]
    fn main_panel_terminal_size_fullscreen_uses_full_width() {
        // Fullscreen: no sidebar, sidebar_width ignored
        // Main width = 80, inner = 80 - 2 = 78
        // Height same as above = 18
        let (cols, rows) = main_panel_terminal_size(80, 24, true, 20, false);
        assert_eq!(cols, 78);
        assert_eq!(rows, 18);
    }

    #[test]
    fn main_panel_terminal_size_zen_mode_uses_full_frame() {
        // Zen mode: terminal fills the entire frame with no borders.
        let (cols, rows) = main_panel_terminal_size(80, 24, false, 20, true);
        assert_eq!(cols, 80);
        assert_eq!(rows, 24);
    }

    #[test]
    fn layout_rects_fullscreen_has_no_sidebar() {
        let (_, sidebar, _, _) = layout_rects(80, 24, true, 20, false);
        assert!(sidebar.is_none());
    }

    #[test]
    fn layout_rects_normal_has_sidebar() {
        let (_, sidebar, _, _) = layout_rects(80, 24, false, 20, false);
        assert!(sidebar.is_some());
        assert_eq!(sidebar.unwrap().width, 20);
    }

    #[test]
    fn layout_rects_zen_mode_main_is_full_frame() {
        let (_, sidebar, main, _) = layout_rects(80, 24, false, 20, true);
        assert!(sidebar.is_none());
        assert_eq!(main.width, 80);
        assert_eq!(main.height, 24);
    }

    #[test]
    fn sidebar_width_is_configurable() {
        let (_, sidebar, _, _) = layout_rects(80, 24, false, 30, false);
        assert_eq!(sidebar.unwrap().width, 30);

        let (cols, _) = main_panel_terminal_size(80, 24, false, 30, false);
        // 80 - 30 = 50, inner = 50 - 2 = 48
        assert_eq!(cols, 48);
    }
}
