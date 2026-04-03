use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use crate::app::App;
use crate::session::DebuggerState;

/// Render the variables panel showing local variables when paused.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let border = super::border_style(app, super::Focus::Variables);

    let title = " Variables [Alt+1] ";

    let items: Vec<ListItem> = if let Some(session) = &app.session {
        if let DebuggerState::Paused { paused_frame, .. } = &session.state {
            paused_frame
                .variables
                .iter()
                .map(|var| {
                    let name_span = Span::styled(&var.name, Style::default().fg(Color::Cyan));
                    let type_span = if let Some(ref ty) = var.r#type {
                        Span::styled(format!(": {ty}"), Style::default().fg(Color::DarkGray))
                    } else {
                        Span::raw("")
                    };
                    let value_span = if let Some(ref val) = var.value {
                        Span::styled(format!(" = {val}"), Style::default().fg(Color::White))
                    } else {
                        Span::raw("")
                    };
                    let expandable = if var.variables_reference.is_some_and(|r| r > 0) {
                        Span::styled(" >", Style::default().fg(Color::Yellow))
                    } else {
                        Span::raw("")
                    };

                    ListItem::new(Line::from(vec![
                        Span::raw("  "),
                        name_span,
                        type_span,
                        value_span,
                        expandable,
                    ]))
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
            .title(title),
    );
    frame.render_widget(list, area);
}
