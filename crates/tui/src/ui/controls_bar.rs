use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, AppMode};

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let hints = match app.mode {
        AppMode::NoSession | AppMode::Terminated => vec![
            styled_key("F5", " Start"),
            Span::raw("  "),
            styled_key("?", " Help"),
        ],
        AppMode::Running | AppMode::Initialising => vec![
            styled_key("S-F5", " Stop"),
            Span::raw("  "),
            styled_key("Ctrl+P", " Files"),
        ],
        AppMode::Paused => vec![
            styled_key("F5", " Continue"),
            Span::raw("  "),
            styled_key("F10", " Over"),
            Span::raw("  "),
            styled_key("F11", " In"),
            Span::raw("  "),
            styled_key("S-F11", " Out"),
            Span::raw("  "),
            styled_key("S-F5", " Stop"),
        ],
    };

    let line = Line::from(hints);
    let block = Block::default().borders(Borders::ALL).title(" Controls ");
    let paragraph = Paragraph::new(line).block(block);
    frame.render_widget(paragraph, area);
}

fn styled_key<'a>(key: &'a str, desc: &'a str) -> Span<'a> {
    // Combine key and description into one span for simplicity.
    // Key is bright, description is dimmed.
    // We return a single span per call; the caller composes the line.
    Span::styled(format!("[{key}{desc}]"), Style::default().fg(Color::Yellow))
}
