use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use crate::app::App;
use crate::session::DebuggerState;

/// Render the call stack panel showing stack frames when paused.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let border = super::border_style(app, super::Focus::CallStack);
    let is_focused = app.focus == super::Focus::CallStack;
    let theme = &app.theme;

    let items: Vec<ListItem> = if let Some(session) = &app.session {
        if let DebuggerState::Paused {
            stack,
            paused_frame,
            ..
        } = &session.state
        {
            stack
                .iter()
                .enumerate()
                .map(|(idx, sf)| {
                    let is_current = sf.id == paused_frame.frame.id;
                    let is_selected = is_focused && idx == app.call_stack_cursor;
                    let marker = if is_current { "\u{25b6}" } else { " " };

                    let source_info = sf
                        .source
                        .as_ref()
                        .and_then(|s| s.name.as_deref())
                        .unwrap_or("?");
                    let text = format!("{marker} {}:{} ({})", sf.name, sf.line, source_info);

                    let style = if is_selected {
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                    } else if is_current {
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text_secondary)
                    };
                    ListItem::new(Line::from(Span::styled(text, style)))
                })
                .collect()
        } else {
            vec![ListItem::new(Span::styled(
                "  (not paused)",
                Style::default().fg(theme.text_muted),
            ))]
        }
    } else {
        vec![ListItem::new(Span::styled(
            "  (no session)",
            Style::default().fg(theme.text_muted),
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
