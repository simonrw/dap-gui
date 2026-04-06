use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

/// Render the help overlay as a floating popup.
pub fn render(frame: &mut Frame, keybindings: &config::keybindings::KeybindingConfig) {
    let area = frame.area();

    let popup_width = ((area.width as f32 * 0.7) as u16).min(70).max(40);
    let popup_height = ((area.height as f32 * 0.85) as u16).min(45).max(12);

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

    use config::keybindings::DebugAction;
    let kb_continue = keybindings
        .label(DebugAction::ContinueOrStart)
        .unwrap_or("?");
    let kb_stop = keybindings.label(DebugAction::Stop).unwrap_or("?");
    let kb_restart = keybindings.label(DebugAction::Restart).unwrap_or("?");
    let kb_step_over = keybindings.label(DebugAction::StepOver).unwrap_or("?");
    let kb_step_into = keybindings.label(DebugAction::StepInto).unwrap_or("?");
    let kb_step_out = keybindings.label(DebugAction::StepOut).unwrap_or("?");

    let lines = vec![
        Line::from(Span::styled("── Global ──", section_style)),
        binding_line("q / Ctrl+C", "Quit", key_style, desc_style),
        binding_line("?", "Toggle this help", key_style, desc_style),
        binding_line("Ctrl+P", "Open file picker", key_style, desc_style),
        binding_line("Ctrl+F, /", "Search in file", key_style, desc_style),
        binding_line("Tab / S-Tab", "Cycle focus fwd/back", key_style, desc_style),
        binding_line("Alt+1/2/3", "Switch bottom tab", key_style, desc_style),
        binding_line("Esc", "Close popup / cancel", key_style, desc_style),
        binding_line(
            "z",
            "Toggle zen mode (maximize code)",
            key_style,
            desc_style,
        ),
        Line::from(""),
        Line::from(Span::styled("── Config ──", section_style)),
        binding_line("h / Left", "Previous config", key_style, desc_style),
        binding_line("l / Right", "Next config", key_style, desc_style),
        Line::from(""),
        Line::from(Span::styled("── Debugger ──", section_style)),
        binding_line(
            &kb_continue,
            "Start / Continue / Restart",
            key_style,
            desc_style,
        ),
        binding_line(&kb_stop, "Terminate / Shutdown", key_style, desc_style),
        binding_line(&kb_restart, "Restart session", key_style, desc_style),
        binding_line(&kb_step_over, "Step Over", key_style, desc_style),
        binding_line(&kb_step_into, "Step In", key_style, desc_style),
        binding_line(&kb_step_out, "Step Out", key_style, desc_style),
        Line::from(""),
        Line::from(Span::styled("── Code View ──", section_style)),
        binding_line("j/k, Up/Down", "Move cursor", key_style, desc_style),
        binding_line("Ctrl+D/U", "Half-page down/up", key_style, desc_style),
        binding_line("g / G", "Go to top / bottom", key_style, desc_style),
        binding_line("b", "Toggle breakpoint at cursor", key_style, desc_style),
        binding_line("v", "Toggle visual line selection", key_style, desc_style),
        binding_line("Esc", "Clear selection", key_style, desc_style),
        binding_line("e", "Evaluate line/selection inline", key_style, desc_style),
        binding_line("x", "Clear inline annotations", key_style, desc_style),
        binding_line("n / N", "Next / prev search match", key_style, desc_style),
        Line::from(""),
        Line::from(Span::styled("── Evaluate Expression ──", section_style)),
        binding_line(
            "Ctrl+E",
            "Open evaluate popup (when paused)",
            key_style,
            desc_style,
        ),
        binding_line("Enter", "Evaluate expression", key_style, desc_style),
        binding_line(
            "Up / Down",
            "Navigate expression history",
            key_style,
            desc_style,
        ),
        binding_line("Esc", "Close popup", key_style, desc_style),
        Line::from(""),
        Line::from(Span::styled("── File Browser ──", section_style)),
        binding_line("/ or i", "Start typing to filter", key_style, desc_style),
        binding_line("j/k", "Navigate files", key_style, desc_style),
        binding_line("Enter", "Open selected file", key_style, desc_style),
        Line::from(""),
        Line::from(Span::styled("── Breakpoints ──", section_style)),
        binding_line("j/k", "Navigate breakpoints", key_style, desc_style),
        binding_line("a", "Add breakpoint (file:line)", key_style, desc_style),
        binding_line(
            "d / Del",
            "Delete selected breakpoint",
            key_style,
            desc_style,
        ),
        binding_line("Enter", "Jump to breakpoint in code", key_style, desc_style),
        Line::from(""),
        Line::from(Span::styled("── Variables ──", section_style)),
        binding_line("j/k", "Navigate variables", key_style, desc_style),
        binding_line("Enter / l", "Expand variable", key_style, desc_style),
        binding_line("h", "Collapse variable", key_style, desc_style),
        binding_line("y", "Yank value (clipboard)", key_style, desc_style),
        Line::from(""),
        Line::from(Span::styled("── Call Stack ──", section_style)),
        binding_line("j/k", "Navigate frames", key_style, desc_style),
        binding_line("Enter", "Change scope to frame", key_style, desc_style),
        Line::from(""),
        Line::from(Span::styled("── Output ──", section_style)),
        binding_line("j/k", "Scroll (when not auto)", key_style, desc_style),
        binding_line("Ctrl+S", "Toggle auto-scroll", key_style, desc_style),
        Line::from(""),
        Line::from(Span::styled("── REPL ──", section_style)),
        binding_line(
            "(type)",
            "Input goes directly to REPL",
            key_style,
            desc_style,
        ),
        binding_line("Enter", "Evaluate expression", key_style, desc_style),
        binding_line("Up / Down", "Navigate input history", key_style, desc_style),
    ];

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn binding_line(key: &str, desc: &str, key_style: Style, desc_style: Style) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {key:<16}"), key_style),
        Span::styled(desc.to_string(), desc_style),
    ])
}

fn centered_rect(width: u16, height: u16, outer: Rect) -> Rect {
    let x = outer.x + (outer.width.saturating_sub(width)) / 2;
    let y = outer.y + (outer.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(outer.width), height.min(outer.height))
}
