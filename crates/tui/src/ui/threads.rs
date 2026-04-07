use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::Span,
    widgets::{Block, Borders, List, ListItem},
};

use crate::app::App;

/// Render the threads panel showing active threads.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let border = super::border_style(app, super::Focus::CodeView); // threads don't have focus yet
    let theme = &app.theme;

    let items: Vec<ListItem> = if app.threads.is_empty() {
        vec![ListItem::new(Span::styled(
            "  (none)",
            Style::default().fg(theme.text_muted),
        ))]
    } else {
        app.threads
            .iter()
            .map(|(id, reason)| {
                let text = format!("  Thread {id} ({reason})");
                ListItem::new(Span::styled(
                    text,
                    Style::default().fg(theme.text_secondary),
                ))
            })
            .collect()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border)
            .title(" Threads "),
    );
    frame.render_widget(list, area);
}
