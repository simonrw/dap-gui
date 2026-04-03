use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use crate::app::App;

/// Render the breakpoints panel showing all UI breakpoints.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let border = super::border_style(app, super::Focus::Breakpoints);

    let items: Vec<ListItem> = if app.ui_breakpoints.is_empty() {
        vec![ListItem::new(Span::styled(
            "  (none)",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        let mut bps: Vec<_> = app.ui_breakpoints.iter().collect();
        bps.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));

        bps.iter()
            .map(|bp| {
                let filename = bp.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                let text = format!("  {} {}:{}", "\u{25cf}", filename, bp.line);
                ListItem::new(Line::from(Span::styled(
                    text,
                    Style::default().fg(Color::Red),
                )))
            })
            .collect()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border)
            .title(" Breakpoints "),
    );
    frame.render_widget(list, area);
}
