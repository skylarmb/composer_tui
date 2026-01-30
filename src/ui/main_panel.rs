//! Main panel widget rendering.
//!
//! Renders the main content area (placeholder for MVP).

use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render the main panel with placeholder content.
pub fn render(frame: &mut Frame, area: Rect) {
    let content = Paragraph::new("Main Content Area")
        .centered()
        .block(Block::default().borders(Borders::ALL));

    frame.render_widget(content, area);
}
