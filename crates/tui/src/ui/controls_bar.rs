use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, AppMode};

/// Render the controls bar showing available keybindings for the current state.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let controls = match app.mode {
        AppMode::NoSession => vec![
            control("F5", "Start"),
            control("Ctrl+P", "Files"),
            control("?", "Help"),
            control("q", "Quit"),
        ],
        AppMode::Initialising | AppMode::Running => vec![
            control("Shift+F5", "Stop"),
            control("Ctrl+P", "Files"),
            control("?", "Help"),
            control("q", "Quit"),
        ],
        AppMode::Paused => vec![
            control("F5", "Continue"),
            control("F10", "Step Over"),
            control("F11", "Step In"),
            control("Shift+F11", "Step Out"),
            control("Shift+F5", "Stop"),
            control("?", "Help"),
        ],
        AppMode::Terminated => vec![
            control("F5", "Restart"),
            control("Shift+F5", "Close"),
            control("q", "Quit"),
        ],
    };

    let mut spans = Vec::new();
    for (i, (key, action)) in controls.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ));
        spans.push(Span::styled(
            format!(" {action}"),
            Style::default().fg(Color::Gray),
        ));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(paragraph, area);
}

fn control(key: &str, action: &str) -> (String, String) {
    (key.to_string(), action.to_string())
}
