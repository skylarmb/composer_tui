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
pub fn render(frame: &mut Frame, area: Rect, focused: bool, focused_border_color: Color) {
    let style = if focused {
        Style::default().fg(focused_border_color).bold()
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
            .border_style(border_style(focused, focused_border_color)),
    );

    frame.render_widget(title, area);
}

fn border_style(focused: bool, focused_color: Color) -> Style {
    if focused {
        Style::default().fg(focused_color)
    } else {
        Style::default()
    }
}
