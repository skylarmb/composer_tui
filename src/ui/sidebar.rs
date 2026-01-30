//! Sidebar widget rendering.
//!
//! Renders the workspace list in the left panel.

use ratatui::{
    layout::Rect,
    style::Stylize,
    text::Line,
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

use crate::App;

/// Render the sidebar with workspace list.
///
/// Shows each workspace name, with the selected one highlighted.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    // Build list items from workspaces
    let items: Vec<ListItem> = app
        .workspaces()
        .iter()
        .enumerate()
        .map(|(i, ws)| {
            let content = if i == app.selected_index() {
                // Selected item gets brackets and reverse style
                Line::from(format!("[{}]", ws.name())).reversed()
            } else {
                Line::from(format!(" {} ", ws.name()))
            };
            ListItem::new(content)
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::ALL));

    frame.render_widget(list, area);
}
