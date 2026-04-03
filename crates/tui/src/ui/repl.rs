use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, AppMode};

/// Render the REPL panel with history and input line.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let border = super::border_style(app, super::Focus::Repl);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border)
        .title(" REPL [Alt+3] ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split: history area + input line
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    // History
    let history_height = chunks[0].height as usize;
    let mut lines: Vec<Line> = Vec::new();

    for (input, output, is_error) in &app.repl_history {
        lines.push(Line::from(vec![
            Span::styled(">> ", Style::default().fg(Color::Cyan)),
            Span::styled(input.as_str(), Style::default().fg(Color::White)),
        ]));

        let prefix = if *is_error { "!! " } else { "=> " };
        let color = if *is_error { Color::Red } else { Color::Green };

        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(color)),
            Span::styled(output.as_str(), Style::default().fg(color)),
        ]));
    }

    // Auto-scroll: show last N lines
    let visible_lines = if lines.len() > history_height {
        lines[lines.len() - history_height..].to_vec()
    } else {
        lines
    };

    let history_paragraph = Paragraph::new(visible_lines);
    frame.render_widget(history_paragraph, chunks[0]);

    // Input line
    let is_paused = app.mode == AppMode::Paused;
    let input_line = if is_paused {
        Line::from(vec![
            Span::styled(">> ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{}\u{258f}", app.repl_input),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        Line::from(Span::styled(
            "   (pause to evaluate)",
            Style::default().fg(Color::DarkGray),
        ))
    };

    let input_paragraph = Paragraph::new(input_line);
    frame.render_widget(input_paragraph, chunks[1]);
}
