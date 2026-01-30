//! Header widget rendering.
//!
//! Renders the application title bar at the top of the screen.

use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render the header with centered "composer_tui" title.
pub fn render(frame: &mut Frame, area: Rect, focused: bool) {
    let style = if focused {
        Style::default().fg(Color::Yellow).bold()
    } else {
        Style::default().bold()
    };

    let title = Paragraph::new(if focused {
        "composer_tui [FOCUS]"
    } else {
        "composer_tui"
    })
    .style(style)
    .centered()
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style(focused)),
    );

    frame.render_widget(title, area);
}

fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    }
}
