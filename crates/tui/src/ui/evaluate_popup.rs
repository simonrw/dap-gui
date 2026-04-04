use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::App;

/// Render the evaluate expression popup as a floating overlay.
pub fn render(app: &App, frame: &mut Frame) {
    let area = frame.area();

    let popup_width = ((area.width as f32 * 0.5) as u16).min(60).max(30);
    let popup_height = 8_u16.min(area.height);

    let popup_area = centered_rect(popup_width, popup_height, area);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Evaluate Expression (Ctrl+E) ");

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    // Split inner into: input (1 row) | separator (1 row) | result (remaining)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // input
            Constraint::Length(1), // separator
            Constraint::Min(1),    // result
        ])
        .split(inner);

    // Input line
    let cursor = "\u{258f}"; // ▏
    let input_line = Line::from(vec![
        Span::styled("> ", Style::default().fg(Color::Yellow)),
        Span::styled(
            format!("{}{}", app.evaluate_input, cursor),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(input_line), chunks[0]);

    // Separator
    let sep = Line::from(Span::styled(
        "\u{2500}".repeat(chunks[1].width as usize),
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(Paragraph::new(sep), chunks[1]);

    // Result area
    let result_paragraph = match &app.evaluate_result {
        Some((text, true)) => {
            // Error
            let line = Line::from(vec![
                Span::styled("!! ", Style::default().fg(Color::Red)),
                Span::styled(text.clone(), Style::default().fg(Color::Red)),
            ]);
            Paragraph::new(line).wrap(Wrap { trim: false })
        }
        Some((text, false)) => {
            // Success
            let line = Line::from(vec![
                Span::styled("=> ", Style::default().fg(Color::Green)),
                Span::styled(text.clone(), Style::default().fg(Color::Green)),
            ]);
            Paragraph::new(line).wrap(Wrap { trim: false })
        }
        None => Paragraph::new(Line::from(Span::styled(
            "(press Enter to evaluate)",
            Style::default().fg(Color::DarkGray),
        ))),
    };
    frame.render_widget(result_paragraph, chunks[2]);
}

fn centered_rect(width: u16, height: u16, outer: Rect) -> Rect {
    let x = outer.x + (outer.width.saturating_sub(width)) / 2;
    let y = outer.y + (outer.height.saturating_sub(height)) / 3;
    Rect::new(x, y, width.min(outer.width), height.min(outer.height))
}
