use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::{App, AppMode};

/// Render the status bar at the bottom showing session state and file info.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let mut spans = Vec::new();

    // Mode indicator
    let (mode_text, mode_color) = match app.mode {
        AppMode::NoSession => (" NO SESSION ", Color::DarkGray),
        AppMode::Initialising => (" STARTING ", Color::Yellow),
        AppMode::Running => (" RUNNING ", Color::Green),
        AppMode::Paused => (" PAUSED ", Color::Cyan),
        AppMode::Terminated => (" TERMINATED ", Color::Red),
    };
    spans.push(Span::styled(
        mode_text,
        Style::default()
            .fg(Color::Black)
            .bg(mode_color)
            .add_modifier(Modifier::BOLD),
    ));

    spans.push(Span::raw(" "));

    // Zen mode indicator
    if app.zen_mode {
        spans.push(Span::styled(
            "[Zen: z] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Config name
    if !app.config_names.is_empty() {
        spans.push(Span::styled(
            format!("[{}]", app.config_names[app.selected_config_index]),
            Style::default().fg(Color::Gray),
        ));
        spans.push(Span::raw(" "));
    }

    // Current file + line
    if let Some(path) = &app.code_view.file_path {
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        let line = app.code_view.cursor_line + 1;
        spans.push(Span::styled(
            format!("{filename}:{line}"),
            Style::default().fg(Color::White),
        ));
        spans.push(Span::raw(" "));
    }

    // Error message (takes priority)
    if let Some(err) = &app.status_error {
        spans.push(Span::styled(
            format!("ERROR: {err}"),
            Style::default().fg(Color::Red),
        ));
    } else if let Some(msg) = &app.status_message {
        spans.push(Span::styled(msg.as_str(), Style::default().fg(Color::Gray)));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}
