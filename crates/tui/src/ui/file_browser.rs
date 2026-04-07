use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::App;

/// Render the file browser sidebar for no-session mode.
/// Shows fuzzy-filtered project files with matched character highlighting.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let border = super::border_style(app, super::Focus::CallStack);
    let theme = &app.theme;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border)
        .title(" Files ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

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
    let base_style = Style::default().fg(theme.text).add_modifier(Modifier::BOLD);
    let mut input_spans = vec![Span::styled("> ", Style::default().fg(theme.accent))];
    input_spans.extend(app.file_browser_editor.render_spans(base_style));
    let input_line = Line::from(input_spans);
    frame.render_widget(Paragraph::new(input_line), chunks[0]);

    // Separator
    let sep = Line::from(Span::styled(
        "\u{2500}".repeat(chunks[1].width as usize),
        Style::default().fg(theme.text_muted),
    ));
    frame.render_widget(Paragraph::new(sep), chunks[1]);

    // File list
    let max_visible = chunks[2].height as usize;
    let items: Vec<ListItem> = app
        .file_browser_results
        .iter()
        .take(max_visible)
        .enumerate()
        .map(|(i, m)| {
            let is_selected = i == app.file_browser_cursor;
            let path_str = m.file.relative_path.to_string_lossy();

            let base_style = if is_selected {
                Style::default()
                    .fg(theme.text)
                    .bg(theme.selection_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_secondary)
            };
            let match_style = if is_selected {
                Style::default()
                    .fg(theme.accent)
                    .bg(theme.selection_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            };

            let indicator = if is_selected { "\u{25b8} " } else { "  " };
            let mut spans = vec![Span::styled(indicator, base_style)];

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
        let msg = if app.file_browser_editor.text().is_empty() {
            "No files found"
        } else {
            "No matches"
        };
        frame.render_widget(
            Paragraph::new(msg).style(Style::default().fg(theme.text_muted)),
            chunks[2],
        );
    } else {
        let list = List::new(items);
        frame.render_widget(list, chunks[2]);
    }
}
