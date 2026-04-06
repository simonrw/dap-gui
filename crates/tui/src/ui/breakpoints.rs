use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::App;

/// Render the breakpoints panel showing all UI breakpoints.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let border = super::border_style(app, super::Focus::Breakpoints);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border)
        .title(" Breakpoints [a:add d:del] ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // If we're in breakpoint input mode, show the input field at the bottom
    if app.breakpoint_editing {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);

        render_list(app, frame, chunks[0]);

        let base_style = Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD);
        let mut spans = vec![Span::styled("> ", Style::default().fg(Color::Yellow))];
        spans.extend(app.breakpoint_editor.render_spans(base_style));
        let input_line = Line::from(spans);
        frame.render_widget(Paragraph::new(input_line), chunks[1]);
    } else {
        render_list(app, frame, inner);
    }
}

fn render_list(app: &App, frame: &mut Frame, area: Rect) {
    if app.ui_breakpoints.is_empty() {
        let empty = Paragraph::new(Span::styled(
            "  (none)",
            Style::default().fg(Color::DarkGray),
        ));
        frame.render_widget(empty, area);
        return;
    }

    let mut bps: Vec<_> = app.ui_breakpoints.iter().collect();
    bps.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));

    let is_focused = app.focus == super::Focus::Breakpoints;

    let items: Vec<ListItem> = bps
        .iter()
        .enumerate()
        .map(|(idx, bp)| {
            let filename = bp.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            let marker = "\u{25cf}"; // ●
            let text = format!("  {marker} {filename}:{}", bp.line);

            let is_selected = is_focused && idx == app.breakpoints_cursor;

            let style = if is_selected {
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(Color::Red)
            };

            ListItem::new(Line::from(Span::styled(text, style)))
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, area);
}
