use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::{App, AppMode};

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let (indicator, label, color) = match app.mode {
        AppMode::NoSession => ("\u{25cb}", "READY", Color::DarkGray),
        AppMode::Initialising => ("\u{25cb}", "INITIALISING", Color::Yellow),
        AppMode::Running => ("\u{25b6}", "RUNNING", Color::Green),
        AppMode::Paused => ("\u{25cf}", "PAUSED", Color::Yellow),
        AppMode::Terminated => ("\u{25a0}", "TERMINATED", Color::Red),
    };

    let line = Line::from(vec![Span::styled(
        format!(" {indicator} {label}"),
        Style::default().fg(color),
    )]);

    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}
