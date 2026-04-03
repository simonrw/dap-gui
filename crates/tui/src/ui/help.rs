use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

/// Render the help overlay as a floating popup.
pub fn render(frame: &mut Frame) {
    let area = frame.area();

    let popup_width = ((area.width as f32 * 0.7) as u16).min(70).max(40);
    let popup_height = ((area.height as f32 * 0.8) as u16).min(30).max(12);

    let popup_area = centered_rect(popup_width, popup_height, area);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Help (? to close) ");

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let key_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);
    let section_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    let lines = vec![
        Line::from(Span::styled("── Global ──", section_style)),
        binding_line("q", "Quit", key_style, desc_style),
        binding_line("?", "Toggle this help", key_style, desc_style),
        binding_line("Ctrl+P", "Open file picker", key_style, desc_style),
        binding_line("Ctrl+F, /", "Search in file", key_style, desc_style),
        binding_line("Tab", "Cycle focus forward", key_style, desc_style),
        binding_line("Shift+Tab", "Cycle focus backward", key_style, desc_style),
        binding_line("Alt+1/2/3", "Switch bottom tab", key_style, desc_style),
        binding_line("Esc", "Close popup / cancel", key_style, desc_style),
        Line::from(""),
        Line::from(Span::styled("── Code View ──", section_style)),
        binding_line("j/k, ↑/↓", "Move cursor up/down", key_style, desc_style),
        binding_line("Ctrl+D/U", "Half-page down/up", key_style, desc_style),
        binding_line("g / G", "Go to top / bottom", key_style, desc_style),
        binding_line("n / N", "Next / prev search match", key_style, desc_style),
        Line::from(""),
        Line::from(Span::styled("── File Picker ──", section_style)),
        binding_line("↑/↓", "Navigate results", key_style, desc_style),
        binding_line("Enter", "Open selected file", key_style, desc_style),
        binding_line("Esc", "Close picker", key_style, desc_style),
        Line::from(""),
        Line::from(Span::styled("── Search ──", section_style)),
        binding_line(
            "Enter",
            "Confirm and return to normal",
            key_style,
            desc_style,
        ),
        binding_line("Esc", "Cancel search", key_style, desc_style),
    ];

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn binding_line<'a>(key: &'a str, desc: &'a str, key_style: Style, desc_style: Style) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("  {key:<16}"), key_style),
        Span::styled(desc, desc_style),
    ])
}

fn centered_rect(width: u16, height: u16, outer: Rect) -> Rect {
    let x = outer.x + (outer.width.saturating_sub(width)) / 2;
    let y = outer.y + (outer.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(outer.width), height.min(outer.height))
}
