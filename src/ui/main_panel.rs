//! Main panel widget rendering.
//!
//! Renders the selected workspace terminal screen.

use ratatui::{
    layout::Rect,
    style::{Color as TuiColor, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{
    terminal::{CellStyle, Color, ScreenBuffer},
    workspace::WorkspaceTerminalState,
    App,
};

/// Render the main panel.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    focused: bool,
    focused_border_color: TuiColor,
) {
    let title = main_title_line(app);
    let lines = if let Some(workspace) = app.selected_workspace() {
        workspace_lines(workspace, focused)
    } else {
        vec![Line::from("No workspace selected")]
    };

    let content = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style(focused, focused_border_color)),
    );

    frame.render_widget(content, area);
}

/// Resolve a tab index from a click on the main-panel top border.
pub fn tab_index_at(area: Rect, app: &App, col: u16, row: u16) -> Option<usize> {
    if row != area.y {
        return None;
    }
    let workspace = app.selected_workspace()?;
    if workspace.tab_count() <= 1 {
        return None;
    }

    let mut x = area.x.saturating_add(1);
    let right = area.x.saturating_add(area.width.saturating_sub(1));
    for fragment in tab_fragments(workspace) {
        let end = x.saturating_add(fragment.text.len() as u16);
        if col >= x && col < end && col < right {
            return Some(fragment.index);
        }
        x = end;
        if x < right {
            x = x.saturating_add(1); // separator '-'
        }
    }
    None
}

fn main_title_line(app: &App) -> Line<'static> {
    let Some(workspace) = app.selected_workspace() else {
        return Line::from("Terminal");
    };
    let fragments = tab_fragments(workspace);
    if fragments.is_empty() {
        return Line::from("Terminal");
    }

    let mut spans = Vec::new();
    for (idx, fragment) in fragments.iter().enumerate() {
        let style = if fragment.active {
            Style::default()
                .fg(TuiColor::Black)
                .bg(TuiColor::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(TuiColor::Gray)
        };
        spans.push(Span::styled(fragment.text.clone(), style));
        if idx + 1 < fragments.len() {
            spans.push(Span::styled(
                "-".to_string(),
                Style::default().fg(TuiColor::DarkGray),
            ));
        }
    }
    Line::from(spans)
}

#[derive(Debug, Clone)]
struct TabFragment {
    index: usize,
    text: String,
    active: bool,
}

fn tab_fragments(workspace: &crate::Workspace) -> Vec<TabFragment> {
    if workspace.tab_count() == 0 {
        return Vec::new();
    }

    (0..workspace.tab_count())
        .map(|index| {
            let label = workspace
                .tab_title(index)
                .unwrap_or_else(|| format!("tab{}", index + 1));
            TabFragment {
                index,
                text: format!("[{label}]"),
                active: index == workspace.active_tab_index(),
            }
        })
        .collect()
}

fn border_style(focused: bool, focused_color: TuiColor) -> Style {
    if focused {
        Style::default().fg(focused_color)
    } else {
        Style::default()
    }
}

fn workspace_lines(workspace: &crate::Workspace, focused: bool) -> Vec<Line<'static>> {
    let show_cursor = focused
        && matches!(workspace.terminal_state(), WorkspaceTerminalState::Running)
        && !workspace.is_scrolled();

    let mut lines = if let Some(screen) = workspace.terminal_screen() {
        let cursor = show_cursor.then(|| screen.cursor_position());
        screen_to_lines(screen, workspace.scroll_offset(), cursor)
    } else {
        let mut base = Vec::new();
        base.push(Line::from(format!("Workspace: {}", workspace.name())));
        if let Some(branch) = workspace.branch_name() {
            base.push(Line::from(format!("Branch: {branch}")));
        }
        if let Some(path) = workspace.worktree_path() {
            base.push(Line::from(format!("Path: {}", path.display())));
        }
        base.push(Line::from(""));
        base.push(Line::from("Initializing terminal..."));
        base
    };

    match workspace.terminal_state() {
        WorkspaceTerminalState::Failed => {
            if lines.is_empty() {
                lines.push(Line::from("Terminal unavailable."));
            }
            if let Some(error) = workspace.terminal_error() {
                lines.push(Line::from(""));
                lines.push(Line::from(format!("Error: {error}")));
            }
        }
        WorkspaceTerminalState::Exited => {
            let status = workspace
                .terminal_exit_status()
                .map(|status| {
                    if let Some(signal) = status.signal() {
                        format!("signal: {signal}")
                    } else {
                        format!("exit code: {}", status.exit_code())
                    }
                })
                .unwrap_or_else(|| "exited".to_string());
            lines.push(Line::from(""));
            lines.push(Line::from(format!("[terminal exited: {status}]")));
        }
        WorkspaceTerminalState::NotStarted | WorkspaceTerminalState::Running => {}
    }

    lines
}

fn screen_to_lines(
    screen: &ScreenBuffer,
    scroll_offset: usize,
    cursor: Option<(usize, usize)>,
) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(screen.rows());
    for (row, cells) in screen.viewport_rows(scroll_offset).enumerate() {
        if cells.is_empty() {
            lines.push(Line::default());
            continue;
        }

        let mut spans = Vec::new();
        let mut current_style = (
            cells[0].style,
            cursor.is_some_and(|(cursor_row, cursor_col)| cursor_row == row && cursor_col == 0),
        );
        let mut current_text = String::new();

        for (col, cell) in cells.iter().enumerate() {
            let style = (
                cell.style,
                cursor
                    .is_some_and(|(cursor_row, cursor_col)| cursor_row == row && cursor_col == col),
            );

            if style != current_style && !current_text.is_empty() {
                spans.push(Span::styled(
                    std::mem::take(&mut current_text),
                    terminal_style(current_style),
                ));
                current_style = style;
            }
            current_text.push(cell.ch);
        }

        if !current_text.is_empty() {
            spans.push(Span::styled(current_text, terminal_style(current_style)));
        }

        lines.push(Line::from(spans));
    }
    lines
}

fn terminal_style((style, cursor): (CellStyle, bool)) -> Style {
    let out = Style::default()
        .fg(map_color(style.fg))
        .bg(map_color(style.bg));
    let mut modifiers = Modifier::empty();
    if style.bold {
        modifiers |= Modifier::BOLD;
    }
    if style.italic {
        modifiers |= Modifier::ITALIC;
    }
    if style.underline {
        modifiers |= Modifier::UNDERLINED;
    }
    if cursor {
        modifiers |= Modifier::REVERSED;
    }
    out.add_modifier(modifiers)
}

fn map_color(color: Color) -> TuiColor {
    match color {
        Color::Default => TuiColor::Reset,
        Color::Indexed(index) => TuiColor::Indexed(index),
        Color::Rgb(r, g, b) => TuiColor::Rgb(r, g, b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_lines_highlight_cursor_cell_when_requested() {
        let mut screen = ScreenBuffer::new(3, 1);
        screen.write(b"ab");

        let lines = screen_to_lines(&screen, 0, Some((0, 2)));
        let cursor_span = lines[0]
            .spans
            .iter()
            .find(|span| span.content.as_ref() == " ")
            .expect("cursor span should exist");
        assert!(cursor_span.style.add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn screen_lines_do_not_highlight_cursor_when_absent() {
        let mut screen = ScreenBuffer::new(3, 1);
        screen.write(b"ab");

        let lines = screen_to_lines(&screen, 0, None);
        assert!(lines[0]
            .spans
            .iter()
            .all(|span| !span.style.add_modifier.contains(Modifier::REVERSED)));
    }

    #[test]
    fn screen_lines_render_from_scrollback_when_offset_is_nonzero() {
        let mut screen = ScreenBuffer::new(4, 2);
        screen.write(b"L1aa\r\nL2bb\r\nL3cc\r\nL4dd");

        let lines = screen_to_lines(&screen, 1, None);
        assert_eq!(lines[0].spans[0].content.as_ref(), "L2bb");
        assert_eq!(lines[1].spans[0].content.as_ref(), "L3cc");
    }

    #[test]
    fn tab_index_at_maps_main_border_clicks() {
        let mut app = App::from_state_with_manager(crate::AppState::default(), None);
        app.add_tab_to_selected_workspace();
        app.add_tab_to_selected_workspace();
        let area = Rect::new(20, 3, 80, 20);

        // First tab starts at area.x + 1 on top border row.
        let col = area.x + 2;
        let row = area.y;
        assert_eq!(tab_index_at(area, &app, col, row), Some(0));
    }
}
