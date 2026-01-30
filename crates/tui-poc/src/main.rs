use std::collections::HashSet;
use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

#[derive(Default, Clone, Copy, PartialEq)]
enum PanelFocus {
    #[default]
    LeftPanel,
    CodeWindow,
    BottomPanel,
}

#[derive(Default, Clone, Copy, PartialEq)]
enum DebugState {
    #[default]
    Stopped,
    Running,
}

struct StackFrame {
    name: &'static str,
    line: usize,
}

impl PanelFocus {
    fn next(self) -> Self {
        match self {
            PanelFocus::LeftPanel => PanelFocus::CodeWindow,
            PanelFocus::CodeWindow => PanelFocus::BottomPanel,
            PanelFocus::BottomPanel => PanelFocus::LeftPanel,
        }
    }
}

struct App {
    focus: PanelFocus,
    debug_state: DebugState,
    code_lines: Vec<&'static str>,
    current_line: usize,
    breakpoints: HashSet<usize>,
    call_stack: Vec<StackFrame>,
    variables: Vec<(&'static str, String)>,
    state_input: String,
    state_output: Vec<String>,
    exit: bool,
}

impl Default for App {
    fn default() -> Self {
        let code_lines = vec![
            "fn main() {",
            "    let x = 10;",
            "    let y = 20;",
            "    let result = process(x, y);",
            "    println!(\"{}\", result);",
            "}",
            "",
            "fn process(a: i32, b: i32) -> i32 {",
            "    validate(a);",
            "    validate(b);",
            "    a + b",
            "}",
            "",
            "fn validate(n: i32) {",
            "    assert!(n > 0);",
            "}",
        ];

        let mut breakpoints = HashSet::new();
        breakpoints.insert(5);
        breakpoints.insert(12);

        let call_stack = vec![StackFrame {
            name: "main",
            line: 1,
        }];

        let variables = vec![("x", "10".to_string()), ("y", "20".to_string())];

        Self {
            focus: PanelFocus::default(),
            debug_state: DebugState::Stopped,
            code_lines,
            current_line: 1,
            breakpoints,
            call_stack,
            variables,
            state_input: String::new(),
            state_output: vec!["Debugger ready. Press F9 to continue.".to_string()],
            exit: false,
        }
    }
}

impl App {
    fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                return Ok(());
            }

            match key.code {
                KeyCode::Char('q') if self.focus != PanelFocus::BottomPanel => {
                    self.exit = true;
                }
                KeyCode::F(7) if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    self.step_out();
                }
                KeyCode::F(7) => {
                    self.step_into();
                }
                KeyCode::F(8) => {
                    self.step_over();
                }
                KeyCode::F(9) => {
                    self.continue_execution();
                }
                KeyCode::Tab => {
                    self.focus = self.focus.next();
                }
                KeyCode::Esc => {
                    if self.focus == PanelFocus::BottomPanel {
                        self.focus = PanelFocus::LeftPanel;
                    }
                }
                KeyCode::Char(c) if self.focus == PanelFocus::BottomPanel => {
                    self.state_input.push(c);
                }
                KeyCode::Backspace if self.focus == PanelFocus::BottomPanel => {
                    self.state_input.pop();
                }
                KeyCode::Enter if self.focus == PanelFocus::BottomPanel => {
                    self.execute_command();
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn execute_command(&mut self) {
        if self.state_input.is_empty() {
            return;
        }
        let cmd = self.state_input.clone();
        self.state_output.push(format!("> {}", cmd));

        let response = match cmd.trim() {
            "help" => "Commands: help, vars, stack, break <n>, clear <n>".to_string(),
            "vars" => self
                .variables
                .iter()
                .map(|(k, v)| format!("  {} = {}", k, v))
                .collect::<Vec<_>>()
                .join("\n"),
            "stack" => self
                .call_stack
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let prefix = if i == 0 { "→" } else { " " };
                    format!("{} {} (line {})", prefix, f.name, f.line)
                })
                .collect::<Vec<_>>()
                .join("\n"),
            other if other.starts_with("break ") => {
                if let Ok(n) = other[6..].trim().parse::<usize>() {
                    self.breakpoints.insert(n);
                    format!("Breakpoint set at line {}", n)
                } else {
                    "Usage: break <line_number>".to_string()
                }
            }
            other if other.starts_with("clear ") => {
                if let Ok(n) = other[6..].trim().parse::<usize>() {
                    if self.breakpoints.remove(&n) {
                        format!("Breakpoint cleared at line {}", n)
                    } else {
                        format!("No breakpoint at line {}", n)
                    }
                } else {
                    "Usage: clear <line_number>".to_string()
                }
            }
            _ => format!("Unknown command: {}", cmd),
        };
        self.state_output.push(response);
        self.state_input.clear();
    }

    fn step_into(&mut self) {
        self.debug_state = DebugState::Stopped;
        let line = self.code_lines.get(self.current_line.saturating_sub(1));

        if let Some(code) = line {
            if code.contains("process(") {
                self.call_stack.push(StackFrame {
                    name: "process",
                    line: self.current_line,
                });
                self.current_line = 8;
                self.variables.push(("a", "10".to_string()));
                self.variables.push(("b", "20".to_string()));
                self.state_output.push("Step into: process()".to_string());
                return;
            } else if code.contains("validate(") {
                self.call_stack.push(StackFrame {
                    name: "validate",
                    line: self.current_line,
                });
                self.current_line = 14;
                self.variables.push(("n", "10".to_string()));
                self.state_output.push("Step into: validate()".to_string());
                return;
            }
        }

        self.advance_line();
        self.state_output
            .push(format!("Step into: line {}", self.current_line));
    }

    fn step_out(&mut self) {
        self.debug_state = DebugState::Stopped;
        if self.call_stack.len() > 1 {
            if let Some(frame) = self.call_stack.pop() {
                self.current_line = frame.line;
                self.advance_line();
                self.variables.retain(|(name, _)| {
                    !matches!(*name, "a" | "b" | "n")
                        || self.call_stack.iter().any(|f| f.name == "process")
                });
                self.state_output.push(format!(
                    "Step out: returned to {} at line {}",
                    self.call_stack.last().map(|f| f.name).unwrap_or("main"),
                    self.current_line
                ));
            }
        } else {
            self.state_output
                .push("Cannot step out: at top of call stack".to_string());
        }
    }

    fn step_over(&mut self) {
        self.debug_state = DebugState::Stopped;
        self.advance_line();
        self.update_variables_for_line();
        self.state_output
            .push(format!("Step over: line {}", self.current_line));
    }

    fn continue_execution(&mut self) {
        self.debug_state = DebugState::Running;
        self.state_output.push("Continuing...".to_string());

        let start = self.current_line;
        loop {
            self.advance_line();
            self.update_variables_for_line();

            if self.breakpoints.contains(&self.current_line) {
                self.debug_state = DebugState::Stopped;
                self.state_output
                    .push(format!("Hit breakpoint at line {}", self.current_line));
                return;
            }

            if self.current_line == start || self.current_line >= self.code_lines.len() {
                self.debug_state = DebugState::Stopped;
                self.state_output
                    .push("No more breakpoints hit".to_string());
                return;
            }
        }
    }

    fn advance_line(&mut self) {
        let current_code = self
            .code_lines
            .get(self.current_line.saturating_sub(1))
            .unwrap_or(&"");

        if current_code.trim() == "}" {
            if self.call_stack.len() > 1 {
                if let Some(frame) = self.call_stack.pop() {
                    self.current_line = frame.line + 1;
                    return;
                }
            }
        }

        self.current_line += 1;
        if self.current_line > self.code_lines.len() {
            self.current_line = 1;
        }

        let new_code = self
            .code_lines
            .get(self.current_line.saturating_sub(1))
            .unwrap_or(&"");
        if new_code.trim().is_empty()
            || new_code.trim().starts_with("fn ")
            || new_code.trim() == "}"
        {
            if self.current_line < self.code_lines.len() {
                self.current_line += 1;
            }
        }
    }

    fn update_variables_for_line(&mut self) {
        match self.current_line {
            2 => {
                if !self.variables.iter().any(|(n, _)| *n == "x") {
                    self.variables.push(("x", "10".to_string()));
                }
            }
            3 => {
                if !self.variables.iter().any(|(n, _)| *n == "y") {
                    self.variables.push(("y", "20".to_string()));
                }
            }
            4 | 5 => {
                if !self.variables.iter().any(|(n, _)| *n == "result") {
                    self.variables.push(("result", "30".to_string()));
                }
            }
            _ => {}
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let outer = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(8),
        ])
        .split(frame.area());

        let middle =
            Layout::horizontal([Constraint::Length(20), Constraint::Min(40)]).split(outer[1]);

        self.render_header(frame, outer[0]);
        self.render_left_panel(frame, middle[0]);
        self.render_code_window(frame, middle[1]);
        self.render_bottom_panel(frame, outer[2]);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let (status, status_color) = match self.debug_state {
            DebugState::Stopped => ("[STOPPED]", Color::Yellow),
            DebugState::Running => ("[RUNNING]", Color::Green),
        };
        let title = "Debugger";
        let keys = "F7:into  S-F7:out  F8:over  F9:cont";
        let padding = area
            .width
            .saturating_sub((title.len() + status.len() + keys.len() + 4) as u16)
            as usize;
        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled(title, Style::default().bold()),
            Span::raw("  "),
            Span::styled(keys, Style::default().dim()),
            Span::raw(" ".repeat(padding)),
            Span::styled(status, Style::default().fg(status_color)),
            Span::raw(" "),
        ]);
        frame.render_widget(
            Paragraph::new(line).style(Style::default().bg(Color::DarkGray)),
            area,
        );
    }

    fn render_left_panel(&self, frame: &mut Frame, area: Rect) {
        let focused = self.focus == PanelFocus::LeftPanel;
        let style = if focused {
            Style::default()
        } else {
            Style::default().dim()
        };

        let bp_count = self.breakpoints.len().max(2) as u16;
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(bp_count),
            Constraint::Length(1),
            Constraint::Min(2),
        ])
        .split(area);

        let bp_title = if focused {
            Line::from("Breakpoints".bold())
        } else {
            Line::from("Breakpoints".dim())
        };
        frame.render_widget(Paragraph::new(bp_title).style(style), chunks[0]);

        let mut bp_lines: Vec<Line> = self
            .breakpoints
            .iter()
            .map(|&line| {
                Line::from(vec![
                    Span::styled(" ● ", Style::default().fg(Color::Red)),
                    Span::raw(format!("line {}", line)),
                ])
            })
            .collect();
        bp_lines.sort_by_key(|l| l.to_string());
        frame.render_widget(Paragraph::new(bp_lines).style(style), chunks[1]);

        let cs_title = if focused {
            Line::from("Call Stack".bold())
        } else {
            Line::from("Call Stack".dim())
        };
        frame.render_widget(Paragraph::new(cs_title).style(style), chunks[2]);

        let cs_lines: Vec<Line> = self
            .call_stack
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let prefix = if i == 0 { "→ " } else { "  " };
                Line::from(format!("{}{}:{}", prefix, f.name, f.line))
            })
            .collect();
        frame.render_widget(Paragraph::new(cs_lines).style(style), chunks[3]);
    }

    fn render_code_window(&self, frame: &mut Frame, area: Rect) {
        let focused = self.focus == PanelFocus::CodeWindow;
        let style = if focused {
            Style::default()
        } else {
            Style::default().dim()
        };

        let lines: Vec<Line> = self
            .code_lines
            .iter()
            .enumerate()
            .map(|(idx, &code)| {
                let line_num = idx + 1;
                let is_current = line_num == self.current_line;
                let has_breakpoint = self.breakpoints.contains(&line_num);

                let bp_marker = if has_breakpoint {
                    Span::styled("●", Style::default().fg(Color::Red))
                } else {
                    Span::raw(" ")
                };

                let line_indicator = if is_current { ">>" } else { "  " };

                let num_span = Span::styled(
                    format!("{:3}", line_num),
                    Style::default().fg(Color::DarkGray),
                );

                let code_style = if is_current {
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                Line::from(vec![
                    bp_marker,
                    Span::raw(" "),
                    Span::styled(line_indicator, Style::default().fg(Color::Yellow)),
                    Span::raw(" "),
                    num_span,
                    Span::raw(" │ "),
                    Span::styled(code.to_string(), code_style),
                ])
            })
            .collect();

        frame.render_widget(Paragraph::new(lines).style(style), area);
    }

    fn render_bottom_panel(&self, frame: &mut Frame, area: Rect) {
        let focused = self.focus == PanelFocus::BottomPanel;
        let style = if focused {
            Style::default()
        } else {
            Style::default().dim()
        };

        let separator = "─".repeat(area.width as usize);
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

        frame.render_widget(
            Paragraph::new(separator).style(Style::default().fg(Color::DarkGray)),
            chunks[0],
        );

        let vars_header = Line::from(vec![
            Span::styled("Variables: ", Style::default().bold()),
            Span::raw(
                self.variables
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        ]);

        let output_height = chunks[1].height.saturating_sub(1) as usize;
        let start = self.state_output.len().saturating_sub(output_height);
        let mut visible_output: Vec<Line> = vec![vars_header];
        visible_output.extend(
            self.state_output[start..]
                .iter()
                .map(|s| Line::from(s.as_str())),
        );
        frame.render_widget(Paragraph::new(visible_output).style(style), chunks[1]);

        let cursor = if focused { "_" } else { "" };
        let input_line = Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Green)),
            Span::raw(&self.state_input),
            Span::styled(cursor, Style::default().add_modifier(Modifier::SLOW_BLINK)),
        ]);
        frame.render_widget(Paragraph::new(input_line).style(style), chunks[2]);
    }
}

fn main() -> io::Result<()> {
    ratatui::run(|terminal| App::default().run(terminal))
}
