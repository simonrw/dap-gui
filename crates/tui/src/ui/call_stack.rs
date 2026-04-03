use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use crate::app::App;
use crate::session::DebuggerState;

/// Render the call stack panel showing stack frames when paused.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let border = super::border_style(app, super::Focus::CallStack);

    let items: Vec<ListItem> = if let Some(session) = &app.session {
        if let DebuggerState::Paused {
            stack,
            paused_frame,
            ..
        } = &session.state
        {
            stack
                .iter()
                .map(|sf| {
                    let is_current = sf.id == paused_frame.frame.id;
                    let marker = if is_current { ">" } else { " " };

                    let source_info = sf
                        .source
                        .as_ref()
                        .and_then(|s| s.name.as_deref())
                        .unwrap_or("?");
                    let text = format!("{marker} {}:{} ({})", sf.name, sf.line, source_info);

                    let style = if is_current {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Gray)
                    };
                    ListItem::new(Line::from(Span::styled(text, style)))
                })
                .collect()
        } else {
            vec![ListItem::new(Span::styled(
                "  (not paused)",
                Style::default().fg(Color::DarkGray),
            ))]
        }
    } else {
        vec![ListItem::new(Span::styled(
            "  (no session)",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border)
            .title(" Call Stack "),
    );
    frame.render_widget(list, area);
}
