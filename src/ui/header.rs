//! Header widget rendering.
//!
//! Renders selected-workspace git/PR summary at the top of the screen.

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{gh_status::GhCiStatus, App, Workspace};

/// Render the top header bar.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    focused: bool,
    focused_border_color: Color,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style(focused, focused_border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let left_text = header_left_text(app);
    let right = header_right_summary(app);
    let right_plain = format!("{} | {}", right.git_text, right.pr_text);
    let right_width = right_plain.chars().count().min(inner.width as usize) as u16;

    let chunks =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(right_width)]).split(inner);

    let left_style = if focused {
        Style::default()
            .fg(focused_border_color)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    };

    frame.render_widget(
        Paragraph::new(left_text)
            .style(left_style)
            .alignment(Alignment::Left),
        chunks[0],
    );

    frame.render_widget(
        Paragraph::new(header_right_line(&right)).alignment(Alignment::Right),
        chunks[1],
    );
}

fn header_left_text(app: &App) -> String {
    match app.selected_workspace() {
        Some(workspace) => format!("composer_tui | {}", workspace_label(workspace)),
        None => "composer_tui | no workspace selected".to_string(),
    }
}

fn workspace_label(workspace: &Workspace) -> String {
    match workspace.branch_name() {
        Some(branch) if branch != workspace.name() => format!("{} ({branch})", workspace.name()),
        _ => workspace.name().to_string(),
    }
}

struct HeaderRightSummary {
    git_text: String,
    git_color: Color,
    pr_text: String,
    pr_color: Color,
}

fn header_right_summary(app: &App) -> HeaderRightSummary {
    let Some(workspace) = app.selected_workspace() else {
        return HeaderRightSummary {
            git_text: "git: n/a".to_string(),
            git_color: Color::Gray,
            pr_text: "pr: n/a".to_string(),
            pr_color: Color::Gray,
        };
    };

    HeaderRightSummary {
        git_text: git_summary_text(workspace),
        git_color: git_summary_color(workspace),
        pr_text: pr_summary_text(workspace),
        pr_color: pr_summary_color(workspace),
    }
}

fn header_right_line(summary: &HeaderRightSummary) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            summary.git_text.clone(),
            Style::default()
                .fg(summary.git_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | ".to_string()),
        Span::styled(
            summary.pr_text.clone(),
            Style::default()
                .fg(summary.pr_color)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn git_summary_text(workspace: &Workspace) -> String {
    if workspace.worktree_path().is_none() {
        return "git: n/a".to_string();
    }

    match workspace.git_status() {
        Some(status) if !status.dirty => "git: clean".to_string(),
        Some(status) if status.unstaged_added > 0 || status.unstaged_deleted > 0 => {
            format!(
                "git: +{}/-{}",
                status.unstaged_added, status.unstaged_deleted
            )
        }
        Some(_) => "git: dirty".to_string(),
        None => "git: ...".to_string(),
    }
}

fn git_summary_color(workspace: &Workspace) -> Color {
    match workspace.git_status() {
        Some(status) if !status.dirty => Color::Green,
        Some(_) => Color::Yellow,
        None if workspace.worktree_path().is_some() => Color::DarkGray,
        None => Color::Gray,
    }
}

fn pr_summary_text(workspace: &Workspace) -> String {
    if workspace.branch_name().is_none() {
        return "pr: n/a".to_string();
    }

    match workspace.gh_status() {
        Some(status) => format!(
            "pr: #{} [{}]",
            status.number,
            status.pr_state.to_ascii_lowercase()
        ),
        None => "pr: ...".to_string(),
    }
}

fn pr_summary_color(workspace: &Workspace) -> Color {
    match workspace.gh_status() {
        Some(status) => match status.ci_status {
            GhCiStatus::Passing => Color::Green,
            GhCiStatus::Pending => Color::Yellow,
            GhCiStatus::Failing => Color::Red,
        },
        None if workspace.branch_name().is_some() => Color::DarkGray,
        None => Color::Gray,
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
    use crate::{gh_status::GhWorkspaceStatus, git_status::GitWorkspaceStatus};
    use std::path::PathBuf;

    #[test]
    fn workspace_label_avoids_duplicate_branch_when_same_as_name() {
        let workspace =
            Workspace::with_worktree("1", "phase-17", PathBuf::from("/tmp/w1"), "phase-17");
        assert_eq!(workspace_label(&workspace), "phase-17");
    }

    #[test]
    fn git_summary_shows_clean_or_line_counts() {
        let mut workspace =
            Workspace::with_worktree("1", "W1", PathBuf::from("/tmp/w1"), "feature");

        workspace.set_git_status(Some(GitWorkspaceStatus {
            dirty: false,
            unstaged_added: 0,
            unstaged_deleted: 0,
        }));
        assert_eq!(git_summary_text(&workspace), "git: clean");

        workspace.set_git_status(Some(GitWorkspaceStatus {
            dirty: true,
            unstaged_added: 12,
            unstaged_deleted: 3,
        }));
        assert_eq!(git_summary_text(&workspace), "git: +12/-3");
    }

    #[test]
    fn pr_summary_shows_number_and_state() {
        let mut workspace =
            Workspace::with_worktree("1", "W1", PathBuf::from("/tmp/w1"), "feature");
        workspace.set_gh_status(Some(GhWorkspaceStatus {
            number: 1234,
            pr_state: "OPEN".to_string(),
            title: "Add top bar".to_string(),
            ci_status: GhCiStatus::Pending,
        }));

        assert_eq!(pr_summary_text(&workspace), "pr: #1234 [open]");
    }
}
