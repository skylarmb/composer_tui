//! Header widget rendering.
//!
//! Renders the application title bar and workspace tabs at the top.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::App;

const TITLE: &str = "composer_tui";

/// Render the header with title and tabs.
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

    let style = if focused {
        Style::default().fg(focused_border_color).bold()
    } else {
        Style::default().bold()
    };
    let mut spans = vec![Span::styled(TITLE.to_string(), style)];

    let fragments = tab_fragments(app);
    if !fragments.is_empty() {
        spans.push(Span::styled(
            "  ".to_string(),
            Style::default().fg(Color::DarkGray),
        ));
    }
    for (idx, fragment) in fragments.iter().enumerate() {
        let tab_style = if fragment.active {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        spans.push(Span::styled(fragment.text.clone(), tab_style));
        if idx + 1 < fragments.len() {
            spans.push(Span::raw(" "));
        }
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), inner);
}

fn border_style(focused: bool, focused_color: Color) -> Style {
    if focused {
        Style::default().fg(focused_color)
    } else {
        Style::default()
    }
}

/// Resolve a tab index from a click position in header coordinates.
pub fn tab_index_at(area: Rect, app: &App, col: u16, row: u16) -> Option<usize> {
    let inner = inner_area(area);
    if inner.width == 0 || inner.height == 0 || row != inner.y {
        return None;
    }

    let fragments = tab_fragments(app);
    if fragments.is_empty() {
        return None;
    }

    let mut x = inner.x.saturating_add(TITLE.len() as u16).saturating_add(2);
    let right = inner.x.saturating_add(inner.width);
    for (idx, fragment) in fragments.iter().enumerate() {
        let width = fragment.text.len() as u16;
        let end = x.saturating_add(width);
        if col >= x && col < end && col < right {
            return Some(fragment.index);
        }
        x = end;
        if idx + 1 < fragments.len() {
            x = x.saturating_add(1);
        }
    }
    None
}

fn inner_area(area: Rect) -> Rect {
    Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}

#[derive(Debug, Clone)]
struct TabFragment {
    index: usize,
    text: String,
    active: bool,
}

fn tab_fragments(app: &App) -> Vec<TabFragment> {
    let Some(workspace) = app.selected_workspace() else {
        return Vec::new();
    };
    if workspace.tab_count() <= 1 {
        return Vec::new();
    }

    (0..workspace.tab_count())
        .map(|index| {
            let label = workspace
                .tab_title(index)
                .unwrap_or_else(|| format!("tab{}", index + 1));
            let active = index == workspace.active_tab_index();
            let text = if active {
                format!("[{label}]")
            } else {
                format!(" {label} ")
            };
            TabFragment {
                index,
                text,
                active,
            }
        })
        .collect()
}
