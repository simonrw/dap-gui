use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::App;

/// Render the REPL panel.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let border = super::border_style(app, super::Focus::Repl);

    let title = " REPL [Alt+3] ";

    let mut lines: Vec<Line> = Vec::new();

    // Show history
    for (input, output, is_error) in &app.repl_history {
        lines.push(Line::from(vec![
            Span::styled(">>> ", Style::default().fg(Color::Cyan)),
            Span::styled(input.as_str(), Style::default().fg(Color::White)),
        ]));
        let color = if *is_error { Color::Red } else { Color::Green };
        // Split output into lines
        for line in output.lines() {
            lines.push(Line::from(Span::styled(
                format!("  {line}"),
                Style::default().fg(color),
            )));
        }
    }

    // Show current input prompt
    lines.push(Line::from(vec![
        Span::styled(">>> ", Style::default().fg(Color::Cyan)),
        Span::styled(app.repl_input.as_str(), Style::default().fg(Color::White)),
        Span::styled("_", Style::default().fg(Color::DarkGray)),
    ]));

    // Scroll to bottom
    let inner_height = area.height.saturating_sub(2) as usize;
    let skip = lines.len().saturating_sub(inner_height);
    let visible: Vec<Line> = lines.into_iter().skip(skip).collect();

    let paragraph = Paragraph::new(visible).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border)
            .title(title),
    );
    frame.render_widget(paragraph, area);
}
