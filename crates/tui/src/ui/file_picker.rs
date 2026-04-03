use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::app::App;

/// Render the file picker as a floating overlay centred on screen.
pub fn render(app: &App, frame: &mut Frame) {
    let area = frame.area();

    // Compute popup dimensions: 60% width, capped at 80 cols; 60% height, capped at 20 rows.
    let popup_width = ((area.width as f32 * 0.6) as u16).min(80).max(30);
    let popup_height = ((area.height as f32 * 0.6) as u16).min(20).max(8);

    let popup_area = centered_rect(popup_width, popup_height, area);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Find File (Ctrl+P) ");

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    // Split inner into: search input (1 line) | separator (1 line) | results list
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // search input
            Constraint::Length(1), // separator
            Constraint::Min(1),    // results
        ])
        .split(inner);

    // Search input
    let cursor = "▏";
    let input_line = Line::from(vec![
        Span::styled("> ", Style::default().fg(Color::Yellow)),
        Span::styled(
            format!("{}{}", app.file_picker.query, cursor),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(input_line), chunks[0]);

    // Separator
    let sep = Line::from(Span::styled(
        "─".repeat(chunks[1].width as usize),
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(Paragraph::new(sep), chunks[1]);

    // Results list
    let max_visible = chunks[2].height as usize;
    let items: Vec<ListItem> = app
        .file_picker
        .results
        .iter()
        .take(max_visible)
        .enumerate()
        .map(|(i, m)| {
            let is_selected = i == app.file_picker.cursor;
            let path_str = m.file.relative_path.to_string_lossy();

            // Build spans with match character highlighting
            let mut spans: Vec<Span> = Vec::new();
            let base_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Rgb(50, 50, 80))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            let match_style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::Rgb(50, 50, 80))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            };

            // Prefix with selection indicator
            let indicator = if is_selected { "▸ " } else { "  " };
            spans.push(Span::styled(indicator, base_style));

            for (ci, ch) in path_str.char_indices() {
                let style = if m.matched_indices.contains(&ci) {
                    match_style
                } else {
                    base_style
                };
                spans.push(Span::styled(ch.to_string(), style));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    if items.is_empty() {
        let msg = if app.file_picker.query.is_empty() {
            "No files found"
        } else {
            "No matches"
        };
        frame.render_widget(
            Paragraph::new(msg).style(Style::default().fg(Color::DarkGray)),
            chunks[2],
        );
    } else {
        let list = List::new(items);
        frame.render_widget(list, chunks[2]);
    }
}

/// Compute a centered rect of fixed width/height within an outer rect.
fn centered_rect(width: u16, height: u16, outer: Rect) -> Rect {
    let x = outer.x + (outer.width.saturating_sub(width)) / 2;
    let y = outer.y + (outer.height.saturating_sub(height)) / 3; // slightly above centre
    Rect::new(x, y, width.min(outer.width), height.min(outer.height))
}
