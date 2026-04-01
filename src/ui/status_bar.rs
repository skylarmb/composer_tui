//! Status bar widget rendering.
//!
//! Renders context-sensitive keybinding hints in a single row at the bottom
//! of the screen. The hints change based on the current focus area and input mode.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::{App, FocusArea, InputMode};

/// Render the status bar with context-sensitive keybinding hints.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let line = hint_line(app);
    let paragraph =
        Paragraph::new(line).style(Style::default().bg(Color::DarkGray).fg(Color::White));
    frame.render_widget(paragraph, area);
}

/// Build the hint line based on current app state.
///
/// Modal hints take priority over focus-based hints.
fn hint_line(app: &App) -> Line<'static> {
    let mut line = match app.input_mode() {
        InputMode::CreateWorkspace { .. } => Line::from(vec![
            key_span("Enter"),
            desc_span(" create  "),
            key_span("Esc"),
            desc_span(" cancel"),
        ]),
        InputMode::ConfirmDelete { .. } => Line::from(vec![
            key_span("Enter"),
            desc_span(" confirm  "),
            key_span("Esc"),
            desc_span(" cancel"),
        ]),
        InputMode::ConfirmCloseTab => Line::from(vec![
            key_span("Enter"),
            desc_span(" close tab  "),
            key_span("Esc"),
            desc_span(" cancel"),
        ]),
        InputMode::CommitMessage { .. } => Line::from(vec![
            key_span("Enter"),
            desc_span(" commit & push  "),
            key_span("Esc"),
            desc_span(" cancel"),
        ]),
        InputMode::ChangesPanel { .. } => Line::from(vec![
            key_span("g/Esc"),
            desc_span(" close  "),
            key_span("C"),
            desc_span(" commit & push"),
        ]),
        InputMode::Error { .. } => Line::from(vec![
            key_span("Enter"),
            desc_span(" dismiss  "),
            key_span("Esc"),
            desc_span(" dismiss"),
        ]),
        InputMode::Normal => {
            // Focus-based hints when no modal is active.
            match app.focus() {
                FocusArea::Sidebar => sidebar_hints(),
                FocusArea::Main => main_hints(),
                FocusArea::Header => header_hints(),
            }
        }
    };

    if let Some(workspace) = app.selected_workspace() {
        if workspace.is_scrolled() {
            let offset = workspace.scroll_offset();
            let label = if offset == 1 {
                "[+1 line]".to_string()
            } else {
                format!("[+{offset} lines]")
            };
            line.spans.push(desc_span("  "));
            line.spans.push(Span::styled(
                label,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }

    line
}

/// Hints shown when the sidebar is focused.
fn sidebar_hints() -> Line<'static> {
    Line::from(vec![
        key_span("j/k"),
        desc_span(" navigate  "),
        key_span("Shift+J/K"),
        desc_span(" move  "),
        key_span("Enter"),
        desc_span(" focus terminal  "),
        key_span("Ctrl+T"),
        desc_span(" new tab  "),
        key_span("Ctrl+W"),
        desc_span(" close tab  "),
        key_span("Alt+1-9"),
        desc_span(" switch tab  "),
        key_span("n"),
        desc_span(" new  "),
        key_span("d"),
        desc_span(" delete  "),
        key_span("g"),
        desc_span(" changes  "),
        key_span("C"),
        desc_span(" commit  "),
        key_span("z"),
        desc_span(" fullscreen  "),
        key_span("Z"),
        desc_span(" zen  "),
        key_span("S"),
        desc_span(" settings  "),
        key_span("R"),
        desc_span(" reload  "),
        key_span("q"),
        desc_span(" quit"),
    ])
}

/// Hints shown when the main panel (terminal) is focused.
fn main_hints() -> Line<'static> {
    Line::from(vec![
        key_span("Alt+1-9"),
        desc_span(" switch tab  "),
        key_span("Shift+PgUp/PgDn"),
        desc_span(" scroll  "),
        key_span("Ctrl+O"),
        desc_span(" sidebar  "),
        key_span("Ctrl+C"),
        desc_span(" interrupt"),
    ])
}

/// Hints shown when the header is focused.
fn header_hints() -> Line<'static> {
    Line::from(vec![
        key_span("Ctrl+J"),
        desc_span(" body  "),
        key_span("Ctrl+H"),
        desc_span(" sidebar"),
    ])
}

/// Styled span for a key label (yellow bold).
fn key_span(text: &str) -> Span<'static> {
    Span::styled(
        text.to_string(),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
}

/// Styled span for a description label (gray).
fn desc_span(text: &str) -> Span<'static> {
    Span::styled(text.to_string(), Style::default().fg(Color::Gray))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppState;

    #[test]
    fn main_hints_include_scrollback_keys() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        app.focus_right();

        let line = hint_line(&app);
        let text = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(text.contains("Shift+PgUp/PgDn"));
        assert!(text.contains("Alt+1-9"));
    }

    #[test]
    fn sidebar_hints_include_settings_keys() {
        let app = App::from_state_with_manager(AppState::default(), None);
        let line = hint_line(&app);
        let text = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(text.contains("S"));
        assert!(text.contains("settings"));
        assert!(text.contains("R"));
        assert!(text.contains("reload"));
        assert!(text.contains("Ctrl+T"));
        assert!(text.contains("Ctrl+W"));
        assert!(text.contains("Shift+J/K"));
        assert!(text.contains("move"));
    }
}
