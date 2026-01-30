//! Header widget rendering.
//!
//! Renders the application title bar at the top of the screen.

use ratatui::{
    layout::Rect,
    style::{Style, Stylize},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render the header with centered "composer_tui" title.
pub fn render(frame: &mut Frame, area: Rect) {
    let title = Paragraph::new("composer_tui")
        .style(Style::default().bold())
        .centered()
        .block(Block::default().borders(Borders::ALL));

    frame.render_widget(title, area);
}
