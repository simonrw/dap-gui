use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, AppMode};

/// Render the controls bar showing available keybindings for the current state.
/// In NoSession/Terminated modes, also shows the config selector with h/l cycling.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let mut spans = Vec::new();

    // Config selector: shown when no active session
    if matches!(app.mode, AppMode::NoSession | AppMode::Terminated) && !app.config_names.is_empty()
    {
        spans.push(Span::styled(
            " \u{25c0} ",
            Style::default().fg(Color::DarkGray),
        )); // ◀
        spans.push(Span::styled(
            format!(" {} ", app.config_names[app.selected_config_index]),
            Style::default()
                .fg(Color::White)
                .bg(Color::Rgb(50, 50, 80))
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            " \u{25b6} ",
            Style::default().fg(Color::DarkGray),
        )); // ▶
        spans.push(Span::raw("  "));
    }

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
            control("Ctrl+Shift+F5", "Restart"),
            control("Shift+F5", "Close"),
            control("q", "Quit"),
        ],
    };

    for (i, (key, action)) in controls.iter().enumerate() {
        if i > 0 || !spans.is_empty() {
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
