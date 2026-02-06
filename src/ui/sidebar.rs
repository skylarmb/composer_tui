//! Sidebar widget rendering.
//!
//! Renders the workspace list in the left panel.

use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

use crate::App;

/// Render the sidebar with workspace list.
///
/// Shows each workspace name, with the selected one highlighted.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    focused: bool,
    focused_border_color: Color,
) {
    // Build list items from workspaces
    let items: Vec<ListItem> = app
        .workspaces()
        .iter()
        .enumerate()
        .map(|(i, ws)| {
            let label = match ws.branch_name() {
                Some(branch) => format!("{} ({})", ws.name(), branch),
                None => ws.name().to_string(),
            };
            let content = if i == app.selected_index() {
                // Selected item gets brackets and reverse style
                Line::from(format!("[{}]", label)).reversed()
            } else {
                Line::from(format!(" {} ", label))
            };
            ListItem::new(content)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style(focused, focused_border_color)),
    );

    frame.render_widget(list, area);
}

fn border_style(focused: bool, focused_color: Color) -> Style {
    if focused {
        Style::default().fg(focused_color)
    } else {
        Style::default()
    }
}
