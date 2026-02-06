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
pub fn render(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let lines = if let Some(workspace) = app.selected_workspace() {
        workspace_lines(workspace, focused)
    } else {
        vec![Line::from("No workspace selected")]
    };

    let content = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Terminal")
            .border_style(border_style(focused)),
    );

    frame.render_widget(content, area);
}

fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(TuiColor::Yellow)
    } else {
        Style::default()
    }
}

fn workspace_lines(workspace: &crate::Workspace, focused: bool) -> Vec<Line<'static>> {
    let mut lines = if let Some(screen) = workspace.terminal_screen() {
        screen_to_lines(screen)
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

    if focused {
        lines.push(Line::from(""));
        lines.push(Line::from("Ctrl+O: sidebar | Ctrl+C: interrupt"));
    } else {
        lines.push(Line::from(""));
        lines.push(Line::from("Enter: focus terminal"));
    }

    lines
}

fn screen_to_lines(screen: &ScreenBuffer) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(screen.rows());
    for row in 0..screen.rows() {
        let Some(cells) = screen.row_cells(row) else {
            lines.push(Line::default());
            continue;
        };

        if cells.is_empty() {
            lines.push(Line::default());
            continue;
        }

        let mut spans = Vec::new();
        let mut current_style = cells[0].style;
        let mut current_text = String::new();

        for cell in cells {
            if cell.style != current_style && !current_text.is_empty() {
                spans.push(Span::styled(
                    std::mem::take(&mut current_text),
                    terminal_style(current_style),
                ));
                current_style = cell.style;
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

fn terminal_style(style: CellStyle) -> Style {
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
    out.add_modifier(modifiers)
}

fn map_color(color: Color) -> TuiColor {
    match color {
        Color::Default => TuiColor::Reset,
        Color::Indexed(index) => TuiColor::Indexed(index),
        Color::Rgb(r, g, b) => TuiColor::Rgb(r, g, b),
    }
}
