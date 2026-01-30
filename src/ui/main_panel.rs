//! Main panel widget rendering.
//!
//! Renders the main content area (placeholder for MVP).

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render the main panel with placeholder content.
pub fn render(frame: &mut Frame, area: Rect, focused: bool) {
    let text = if focused {
        "Main Content Area [FOCUS]"
    } else {
        "Main Content Area"
    };

    let content = Paragraph::new(text).centered().block(
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
