use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::{App, AppMode};

/// Render the status bar at the bottom showing session state and file info.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let theme = &app.theme;
    let mut spans = Vec::new();

    // Mode indicator
    let (mode_text, mode_color) = match app.mode {
        AppMode::NoSession => (" NO SESSION ", theme.text_muted),
        AppMode::Initialising => (" STARTING ", theme.warning),
        AppMode::Running => (" RUNNING ", theme.success),
        AppMode::Paused => (" PAUSED ", theme.accent_alt),
        AppMode::Terminated => (" TERMINATED ", theme.error),
    };
    spans.push(Span::styled(
        mode_text,
        Style::default()
            .fg(theme.status_badge_fg)
            .bg(mode_color)
            .add_modifier(Modifier::BOLD),
    ));

    spans.push(Span::raw(" "));

    // Zen mode indicator
    if app.zen_mode {
        spans.push(Span::styled(
            "[Zen: z] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Config name
    if !app.config_names.is_empty() {
        spans.push(Span::styled(
            format!("[{}]", app.config_names[app.selected_config_index]),
            Style::default().fg(theme.text_secondary),
        ));
        spans.push(Span::raw(" "));
    }

    // Current file + line
    if let Some(path) = &app.code_view.file_path {
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        let line = app.code_view.cursor_line + 1;
        spans.push(Span::styled(
            format!("{filename}:{line}"),
            Style::default().fg(theme.text),
        ));
        spans.push(Span::raw(" "));
    }

    // Error message (takes priority)
    if let Some(err) = &app.status_error {
        spans.push(Span::styled(
            format!("ERROR: {err}"),
            Style::default().fg(theme.error),
        ));
    } else if let Some(msg) = &app.status_message {
        spans.push(Span::styled(
            msg.as_str(),
            Style::default().fg(theme.text_secondary),
        ));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}
