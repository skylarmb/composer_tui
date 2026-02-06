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
    match app.input_mode() {
        InputMode::CreateWorkspace { .. } => {
            return Line::from(vec![
                key_span("Enter"),
                desc_span(" create  "),
                key_span("Esc"),
                desc_span(" cancel"),
            ]);
        }
        InputMode::ConfirmDelete { .. } => {
            return Line::from(vec![
                key_span("Enter"),
                desc_span(" confirm  "),
                key_span("Esc"),
                desc_span(" cancel"),
            ]);
        }
        InputMode::Error { .. } => {
            return Line::from(vec![
                key_span("Enter"),
                desc_span(" dismiss  "),
                key_span("Esc"),
                desc_span(" dismiss"),
            ]);
        }
        InputMode::Normal => {}
    }

    // Focus-based hints when no modal is active.
    match app.focus() {
        FocusArea::Sidebar => sidebar_hints(),
        FocusArea::Main => main_hints(),
        FocusArea::Header => header_hints(),
    }
}

/// Hints shown when the sidebar is focused.
fn sidebar_hints() -> Line<'static> {
    Line::from(vec![
        key_span("j/k"),
        desc_span(" navigate  "),
        key_span("Enter"),
        desc_span(" focus terminal  "),
        key_span("n"),
        desc_span(" new  "),
        key_span("d"),
        desc_span(" delete  "),
        key_span("z"),
        desc_span(" fullscreen  "),
        key_span("q"),
        desc_span(" quit"),
    ])
}

/// Hints shown when the main panel (terminal) is focused.
fn main_hints() -> Line<'static> {
    Line::from(vec![
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
