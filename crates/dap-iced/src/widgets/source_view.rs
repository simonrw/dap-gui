use std::collections::HashSet;

use iced::widget::{column, container, mouse_area, row, scrollable, text};
use iced::{Color, Element, Fill, Font};

use crate::message::Message;

/// Renders the source code view with line numbers, breakpoints, and current line highlighting.
pub fn source_view<'a>(
    content: &'a str,
    current_line: Option<usize>,
    breakpoints: &'a HashSet<usize>,
) -> Element<'a, Message> {
    if content.is_empty() {
        return container(text("No source file loaded").color(Color::from_rgb(0.5, 0.5, 0.5)))
            .padding(20)
            .into();
    }

    let lines: Vec<Element<Message>> = content
        .lines()
        .enumerate()
        .map(|(idx, line_text)| {
            let line_num = idx + 1; // 1-indexed
            let is_current = current_line == Some(line_num);
            let has_breakpoint = breakpoints.contains(&line_num);

            source_line(line_num, line_text, is_current, has_breakpoint)
        })
        .collect();

    scrollable(column(lines).spacing(0))
        .height(Fill)
        .width(Fill)
        .into()
}

/// Renders a single line of source code.
fn source_line(
    line_num: usize,
    content: &str,
    is_current: bool,
    has_breakpoint: bool,
) -> Element<'_, Message> {
    let bp_indicator = if has_breakpoint { "@" } else { " " };
    let current_marker = if is_current { ">" } else { " " };

    // Line number gutter (clickable for breakpoint toggle)
    let gutter = mouse_area(row![
        text(format!("{:4} ", line_num))
            .font(Font::MONOSPACE)
            .color(Color::from_rgb(0.5, 0.5, 0.5)),
        text(bp_indicator)
            .font(Font::MONOSPACE)
            .color(Color::from_rgb(1.0, 0.2, 0.2)),
        text(format!("{} ", current_marker))
            .font(Font::MONOSPACE)
            .color(Color::from_rgb(1.0, 1.0, 0.0)),
    ])
    .on_press(Message::ToggleBreakpoint(line_num));

    // Code content - use a space for empty lines to maintain height
    let display_content = if content.is_empty() { " " } else { content };
    let code = text(display_content).font(Font::MONOSPACE);

    let line_row = row![gutter, code];

    // Apply background highlight for current line
    if is_current {
        container(line_row)
            .style(|_| container::Style {
                background: Some(Color::from_rgb(0.15, 0.15, 0.05).into()),
                ..Default::default()
            })
            .width(Fill)
            .into()
    } else {
        container(line_row).width(Fill).into()
    }
}
