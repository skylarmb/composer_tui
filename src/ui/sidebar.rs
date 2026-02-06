//! Sidebar widget rendering.
//!
//! Renders the workspace list in the left panel.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

use crate::{
    gh_status::GhCiStatus,
    workspace::{Workspace, WorkspaceTerminalState},
    App,
};

/// Render the sidebar with workspace list.
///
/// Shows each workspace with:
/// - process state dot (`●`)
/// - workspace name + branch
/// - git dirty marker (`*`)
/// - PR badge (`PR`) when gh status is available
pub fn render(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    focused: bool,
    focused_border_color: Color,
) {
    let items: Vec<ListItem> = app
        .workspaces()
        .iter()
        .enumerate()
        .map(|(i, workspace)| {
            let mut item = ListItem::new(workspace_line(workspace));
            if i == app.selected_index() {
                let selected_style = app
                    .config()
                    .selected_bg_color()
                    .map(|bg| Style::default().bg(bg))
                    .unwrap_or_else(|| Style::default().add_modifier(Modifier::REVERSED));
                item = item.style(selected_style);
            }
            item
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style(focused, focused_border_color)),
    );

    frame.render_widget(list, area);
}

fn workspace_line(workspace: &Workspace) -> Line<'static> {
    let mut spans = vec![
        Span::styled(
            "● ".to_string(),
            Style::default().fg(process_status_color(workspace.terminal_state())),
        ),
        Span::raw(workspace.name().to_string()),
    ];

    if let Some(branch) = workspace.branch_name() {
        spans.push(Span::raw(format!(" ({branch})")));
    }

    if workspace.git_status().is_some_and(|status| status.dirty) {
        spans.push(Span::styled(
            " *".to_string(),
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(gh_status) = workspace.gh_status() {
        spans.push(Span::raw(" ".to_string()));
        spans.push(Span::styled(
            "PR".to_string(),
            Style::default()
                .fg(pr_badge_color(gh_status.ci_status))
                .add_modifier(Modifier::BOLD),
        ));
    }

    Line::from(spans)
}

fn process_status_color(state: WorkspaceTerminalState) -> Color {
    match state {
        WorkspaceTerminalState::Running => Color::Green,
        WorkspaceTerminalState::Exited => Color::Red,
        WorkspaceTerminalState::NotStarted => Color::DarkGray,
        WorkspaceTerminalState::Failed => Color::Yellow,
    }
}

fn pr_badge_color(status: GhCiStatus) -> Color {
    match status {
        GhCiStatus::Passing => Color::Green,
        GhCiStatus::Pending => Color::Yellow,
        GhCiStatus::Failing => Color::Red,
    }
}

fn border_style(focused: bool, focused_color: Color) -> Style {
    if focused {
        Style::default().fg(focused_color)
    } else {
        Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        gh_status::GhWorkspaceStatus, git_status::GitWorkspaceStatus, workspace::Workspace,
    };
    use std::path::PathBuf;

    #[test]
    fn process_dot_colors_match_terminal_states() {
        assert_eq!(
            process_status_color(WorkspaceTerminalState::Running),
            Color::Green
        );
        assert_eq!(
            process_status_color(WorkspaceTerminalState::Exited),
            Color::Red
        );
        assert_eq!(
            process_status_color(WorkspaceTerminalState::NotStarted),
            Color::DarkGray
        );
        assert_eq!(
            process_status_color(WorkspaceTerminalState::Failed),
            Color::Yellow
        );
    }

    #[test]
    fn workspace_line_includes_git_and_pr_markers() {
        let mut workspace =
            Workspace::with_worktree("1", "W1", PathBuf::from("/tmp/w1"), "feature/w1");
        workspace.set_git_status(Some(GitWorkspaceStatus {
            dirty: true,
            unstaged_added: 2,
            unstaged_deleted: 1,
        }));
        workspace.set_gh_status(Some(GhWorkspaceStatus {
            number: 1234,
            pr_state: "OPEN".to_string(),
            title: "PR title".to_string(),
            ci_status: GhCiStatus::Passing,
        }));

        let line = workspace_line(&workspace);
        let text = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(text.contains("● W1 (feature/w1)"));
        assert!(text.contains('*'));
        assert!(text.contains("PR"));
    }
}
