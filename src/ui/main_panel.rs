//! Main panel widget rendering.
//!
//! Renders the main content area (placeholder for MVP).

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{App, InputMode};

/// Render the main panel with placeholder content.
pub fn render(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let mut lines = Vec::new();
    if let Some(workspace) = app.selected_workspace() {
        lines.push(format!("Workspace: {}", workspace.name()));
        if let Some(branch) = workspace.branch_name() {
            lines.push(format!("Branch: {branch}"));
        }
        if let Some(path) = workspace.worktree_path() {
            lines.push(format!("Path: {}", path.display()));
        }
    } else {
        lines.push("No workspace selected".to_string());
    }

    lines.push(String::new());
    lines.push("Keys: n=new workspace, d=delete workspace, Enter=confirm, Esc=cancel".to_string());
    lines.push("Navigation: j/k or arrows".to_string());

    if focused {
        lines.push(String::new());
        lines.push("Main panel [FOCUS]".to_string());
    }

    match app.input_mode() {
        InputMode::CreateWorkspace { .. } => lines.push("Create mode active".to_string()),
        InputMode::ConfirmDelete { .. } => lines.push("Delete confirmation active".to_string()),
        InputMode::Error { .. } => lines.push("Error dialog active".to_string()),
        InputMode::Normal => {}
    };

    let content = Paragraph::new(lines.join("\n")).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style(focused)),
    );

    frame.render_widget(content, area);
}

fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    }
}
