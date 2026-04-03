use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::App;

/// Render the output panel showing program stdout/stderr.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let border = super::border_style(app, super::Focus::Output);

    let title = " Output [Alt+2] ";

    let lines: Vec<Line> = if app.output_lines.is_empty() {
        vec![Line::from(Span::styled(
            "  (no output)",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        app.output_lines
            .iter()
            .map(|(category, text)| {
                let color = match category.as_str() {
                    "stderr" => Color::Red,
                    "stdout" => Color::White,
                    "console" => Color::Gray,
                    _ => Color::DarkGray,
                };
                // Trim trailing newlines for display
                let text = text.trim_end_matches('\n');
                Line::from(Span::styled(
                    format!("  {text}"),
                    Style::default().fg(color),
                ))
            })
            .collect()
    };

    // Show the last N lines that fit, scrolled to bottom
    let inner_height = area.height.saturating_sub(2) as usize; // borders
    let skip = lines.len().saturating_sub(inner_height);
    let visible_lines: Vec<Line> = lines.into_iter().skip(skip).collect();

    let paragraph = Paragraph::new(visible_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border)
            .title(title),
    );
    frame.render_widget(paragraph, area);
}
