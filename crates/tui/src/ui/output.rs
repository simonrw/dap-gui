use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::App;

/// Maximum output lines to keep in memory.
pub const MAX_OUTPUT_LINES: usize = 10_000;

/// Render the program output panel with category coloring and auto-scroll.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let border = super::border_style(app, super::Focus::Output);

    let auto_indicator = if app.output_auto_scroll {
        "auto"
    } else {
        "scroll"
    };
    let title = format!(" Output [Alt+2] ({auto_indicator}) ");

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border)
        .title(title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let viewport_height = inner.height as usize;

    if app.output_lines.is_empty() {
        let empty = Paragraph::new(Span::styled(
            "  (no output)",
            Style::default().fg(Color::DarkGray),
        ));
        frame.render_widget(empty, inner);
        return;
    }

    // Build lines with category coloring, filtering out telemetry
    let visible_lines: Vec<Line> = app
        .output_lines
        .iter()
        .filter(|(cat, _)| cat != "telemetry")
        .map(|(category, text)| {
            let color = match category.as_str() {
                "stderr" => Color::Red,
                "console" => Color::DarkGray,
                "important" => Color::Yellow,
                _ => Color::White, // stdout and others
            };

            let prefix = match category.as_str() {
                "stderr" => Span::styled("[err] ", Style::default().fg(Color::Red)),
                "console" => Span::styled("[dbg] ", Style::default().fg(Color::DarkGray)),
                "important" => Span::styled("[!!!] ", Style::default().fg(Color::Yellow)),
                _ => Span::raw(""),
            };

            Line::from(vec![
                prefix,
                Span::styled(text.as_str(), Style::default().fg(color)),
            ])
        })
        .collect();

    // Apply scroll
    let total = visible_lines.len();
    let offset = if app.output_auto_scroll {
        total.saturating_sub(viewport_height)
    } else {
        app.output_scroll_offset
            .min(total.saturating_sub(viewport_height))
    };

    let display_lines: Vec<Line> = visible_lines
        .into_iter()
        .skip(offset)
        .take(viewport_height)
        .collect();

    let paragraph = Paragraph::new(display_lines);
    frame.render_widget(paragraph, inner);
}
