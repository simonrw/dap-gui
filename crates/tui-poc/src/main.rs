use std::collections::HashMap;
use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    DefaultTerminal, Frame,
};

#[derive(Default, Clone, Copy, PartialEq)]
enum PanelFocus {
    #[default]
    CallStack,
    Breakpoints,
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

struct Breakpoint {
    line: usize,
    enabled: bool,
}

struct FileContent {
    lines: Vec<&'static str>,
}

impl PanelFocus {
    fn next(self) -> Self {
        match self {
            PanelFocus::CallStack => PanelFocus::Breakpoints,
            PanelFocus::Breakpoints => PanelFocus::CodeWindow,
            PanelFocus::CodeWindow => PanelFocus::BottomPanel,
            PanelFocus::BottomPanel => PanelFocus::CallStack,
        }
    }

    fn prev(self) -> Self {
        match self {
            PanelFocus::CallStack => PanelFocus::BottomPanel,
            PanelFocus::Breakpoints => PanelFocus::CallStack,
            PanelFocus::CodeWindow => PanelFocus::Breakpoints,
            PanelFocus::BottomPanel => PanelFocus::CodeWindow,
        }
    }
}

struct App {
    focus: PanelFocus,
    debug_state: DebugState,
    files: HashMap<&'static str, FileContent>,
    current_file: &'static str,
    current_line: usize,
    breakpoints: Vec<Breakpoint>,
    breakpoint_cursor: usize,
    call_stack: Vec<StackFrame>,
    call_stack_cursor: usize,
    variables: Vec<(&'static str, String)>,
    state_input: String,
    state_output: Vec<String>,
    exit: bool,
    // Code window cursor (separate from execution position)
    code_cursor_line: usize,
    // Command palette
    command_palette_open: bool,
    command_palette_input: String,
    command_palette_cursor: usize,
    command_palette_filtered: Vec<&'static str>,
    // Adding breakpoint mode
    adding_breakpoint: bool,
    new_breakpoint_input: String,
}

impl Default for App {
    fn default() -> Self {
        let mut files = HashMap::new();

        files.insert(
            "main.rs",
            FileContent {
                lines: vec![
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
                ],
            },
        );

        files.insert(
            "utils.rs",
            FileContent {
                lines: vec![
                    "pub fn helper() -> i32 {",
                    "    42",
                    "}",
                    "",
                    "pub fn format_output(val: i32) -> String {",
                    "    format!(\"Result: {}\", val)",
                    "}",
                ],
            },
        );

        files.insert(
            "config.rs",
            FileContent {
                lines: vec![
                    "pub struct Config {",
                    "    pub debug: bool,",
                    "    pub verbose: bool,",
                    "}",
                    "",
                    "impl Default for Config {",
                    "    fn default() -> Self {",
                    "        Self {",
                    "            debug: false,",
                    "            verbose: false,",
                    "        }",
                    "    }",
                    "}",
                ],
            },
        );

        let breakpoints = vec![
            Breakpoint {
                line: 5,
                enabled: true,
            },
            Breakpoint {
                line: 12,
                enabled: true,
            },
        ];

        let call_stack = vec![StackFrame {
            name: "main",
            line: 1,
        }];

        let variables = vec![("x", "10".to_string()), ("y", "20".to_string())];

        Self {
            focus: PanelFocus::default(),
            debug_state: DebugState::Stopped,
            files,
            current_file: "main.rs",
            current_line: 1,
            breakpoints,
            breakpoint_cursor: 0,
            call_stack,
            call_stack_cursor: 0,
            variables,
            state_input: String::new(),
            state_output: vec!["Debugger ready. Press F9 to continue.".to_string()],
            exit: false,
            code_cursor_line: 1,
            command_palette_open: false,
            command_palette_input: String::new(),
            command_palette_cursor: 0,
            command_palette_filtered: Vec::new(),
            adding_breakpoint: false,
            new_breakpoint_input: String::new(),
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

    fn get_current_file_lines(&self) -> &[&'static str] {
        self.files
            .get(self.current_file)
            .map(|f| f.lines.as_slice())
            .unwrap_or(&[])
    }

    fn get_file_list(&self) -> Vec<&'static str> {
        let mut files: Vec<_> = self.files.keys().copied().collect();
        files.sort();
        files
    }

    fn update_filtered_files(&mut self) {
        let query = self.command_palette_input.to_lowercase();
        self.command_palette_filtered = self
            .get_file_list()
            .into_iter()
            .filter(|f| f.to_lowercase().contains(&query))
            .collect();
        if self.command_palette_cursor >= self.command_palette_filtered.len() {
            self.command_palette_cursor = 0;
        }
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                return Ok(());
            }

            // Handle command palette
            if self.command_palette_open {
                return self.handle_command_palette_input(key.code);
            }

            // Handle adding breakpoint mode
            if self.adding_breakpoint {
                return self.handle_add_breakpoint_input(key.code);
            }

            match key.code {
                KeyCode::Char('q')
                    if self.focus != PanelFocus::BottomPanel && !self.adding_breakpoint =>
                {
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
                KeyCode::BackTab => {
                    self.focus = self.focus.prev();
                }
                KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.open_command_palette();
                }
                KeyCode::Esc => {
                    if self.focus == PanelFocus::BottomPanel {
                        self.focus = PanelFocus::CallStack;
                    }
                }
                // Panel-specific handling
                _ => match self.focus {
                    PanelFocus::CallStack => self.handle_call_stack_input(key.code),
                    PanelFocus::Breakpoints => self.handle_breakpoints_input(key.code),
                    PanelFocus::CodeWindow => self.handle_code_window_input(key.code),
                    PanelFocus::BottomPanel => self.handle_bottom_panel_input(key.code),
                },
            }
        }
        Ok(())
    }

    fn handle_call_stack_input(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.call_stack.is_empty() {
                    self.call_stack_cursor =
                        (self.call_stack_cursor + 1).min(self.call_stack.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.call_stack_cursor = self.call_stack_cursor.saturating_sub(1);
            }
            KeyCode::Enter => {
                // Jump to the selected stack frame
                if let Some(frame) = self.call_stack.get(self.call_stack_cursor) {
                    self.code_cursor_line = frame.line;
                    self.state_output
                        .push(format!("Jumped to {} at line {}", frame.name, frame.line));
                }
            }
            _ => {}
        }
    }

    fn handle_breakpoints_input(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.breakpoints.is_empty() {
                    self.breakpoint_cursor =
                        (self.breakpoint_cursor + 1).min(self.breakpoints.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.breakpoint_cursor = self.breakpoint_cursor.saturating_sub(1);
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                // Toggle enable/disable
                if let Some(bp) = self.breakpoints.get_mut(self.breakpoint_cursor) {
                    bp.enabled = !bp.enabled;
                    let status = if bp.enabled { "enabled" } else { "disabled" };
                    self.state_output
                        .push(format!("Breakpoint at line {} {}", bp.line, status));
                }
            }
            KeyCode::Char('a') => {
                // Add new breakpoint
                self.adding_breakpoint = true;
                self.new_breakpoint_input.clear();
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                // Delete breakpoint
                if !self.breakpoints.is_empty() {
                    let removed = self.breakpoints.remove(self.breakpoint_cursor);
                    self.state_output
                        .push(format!("Removed breakpoint at line {}", removed.line));
                    if self.breakpoint_cursor > 0
                        && self.breakpoint_cursor >= self.breakpoints.len()
                    {
                        self.breakpoint_cursor = self.breakpoints.len().saturating_sub(1);
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_add_breakpoint_input(&mut self, code: KeyCode) -> io::Result<()> {
        match code {
            KeyCode::Esc => {
                self.adding_breakpoint = false;
                self.new_breakpoint_input.clear();
            }
            KeyCode::Enter => {
                if let Ok(line) = self.new_breakpoint_input.trim().parse::<usize>() {
                    // Check if breakpoint already exists
                    if !self.breakpoints.iter().any(|bp| bp.line == line) {
                        self.breakpoints.push(Breakpoint {
                            line,
                            enabled: true,
                        });
                        self.breakpoints.sort_by_key(|bp| bp.line);
                        self.breakpoint_cursor = self
                            .breakpoints
                            .iter()
                            .position(|bp| bp.line == line)
                            .unwrap_or(0);
                        self.state_output
                            .push(format!("Added breakpoint at line {}", line));
                    } else {
                        self.state_output
                            .push(format!("Breakpoint already exists at line {}", line));
                    }
                } else {
                    self.state_output.push("Invalid line number".to_string());
                }
                self.adding_breakpoint = false;
                self.new_breakpoint_input.clear();
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                self.new_breakpoint_input.push(c);
            }
            KeyCode::Backspace => {
                self.new_breakpoint_input.pop();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_code_window_input(&mut self, code: KeyCode) {
        let total_lines = self.get_current_file_lines().len();
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.code_cursor_line < total_lines {
                    self.code_cursor_line += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.code_cursor_line > 1 {
                    self.code_cursor_line -= 1;
                }
            }
            KeyCode::Char('g') => {
                // gg - go to top (simplified: single g goes to top)
                self.code_cursor_line = 1;
            }
            KeyCode::Char('G') => {
                // G - go to bottom
                self.code_cursor_line = total_lines.max(1);
            }
            KeyCode::Char('b') => {
                // Toggle breakpoint at cursor line
                let line = self.code_cursor_line;
                if let Some(idx) = self.breakpoints.iter().position(|bp| bp.line == line) {
                    self.breakpoints.remove(idx);
                    self.state_output
                        .push(format!("Removed breakpoint at line {}", line));
                } else {
                    self.breakpoints.push(Breakpoint {
                        line,
                        enabled: true,
                    });
                    self.breakpoints.sort_by_key(|bp| bp.line);
                    self.state_output
                        .push(format!("Added breakpoint at line {}", line));
                }
            }
            KeyCode::Char('0') => {
                // Return to current execution line
                self.code_cursor_line = self.current_line;
                self.state_output.push(format!(
                    "Jumped to current execution line {}",
                    self.current_line
                ));
            }
            KeyCode::Char('H') => {
                // Half page up
                self.code_cursor_line = self.code_cursor_line.saturating_sub(10).max(1);
            }
            KeyCode::Char('L') => {
                // Half page down
                self.code_cursor_line = (self.code_cursor_line + 10).min(total_lines.max(1));
            }
            KeyCode::Char('{') => {
                // Jump to previous blank line (paragraph up)
                let lines = self.get_current_file_lines();
                let mut target = self.code_cursor_line.saturating_sub(1);
                while target > 1 {
                    if lines
                        .get(target.saturating_sub(1))
                        .map_or(true, |l| l.trim().is_empty())
                    {
                        break;
                    }
                    target -= 1;
                }
                self.code_cursor_line = target.max(1);
            }
            KeyCode::Char('}') => {
                // Jump to next blank line (paragraph down)
                let lines = self.get_current_file_lines();
                let mut target = self.code_cursor_line + 1;
                while target < lines.len() {
                    if lines
                        .get(target.saturating_sub(1))
                        .map_or(true, |l| l.trim().is_empty())
                    {
                        break;
                    }
                    target += 1;
                }
                self.code_cursor_line = target.min(lines.len().max(1));
            }
            _ => {}
        }
    }

    fn handle_bottom_panel_input(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char(c) => {
                self.state_input.push(c);
            }
            KeyCode::Backspace => {
                self.state_input.pop();
            }
            KeyCode::Enter => {
                self.execute_command();
            }
            _ => {}
        }
    }

    fn open_command_palette(&mut self) {
        self.command_palette_open = true;
        self.command_palette_input.clear();
        self.command_palette_cursor = 0;
        self.update_filtered_files();
    }

    fn handle_command_palette_input(&mut self, code: KeyCode) -> io::Result<()> {
        match code {
            KeyCode::Esc => {
                self.command_palette_open = false;
            }
            KeyCode::Enter => {
                if let Some(&file) = self
                    .command_palette_filtered
                    .get(self.command_palette_cursor)
                {
                    self.current_file = file;
                    self.code_cursor_line = 1;
                    self.state_output.push(format!("Opened file: {}", file));
                }
                self.command_palette_open = false;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.command_palette_filtered.is_empty() {
                    self.command_palette_cursor =
                        (self.command_palette_cursor + 1) % self.command_palette_filtered.len();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if !self.command_palette_filtered.is_empty() {
                    self.command_palette_cursor = if self.command_palette_cursor == 0 {
                        self.command_palette_filtered.len() - 1
                    } else {
                        self.command_palette_cursor - 1
                    };
                }
            }
            KeyCode::Char(c) => {
                self.command_palette_input.push(c);
                self.update_filtered_files();
            }
            KeyCode::Backspace => {
                self.command_palette_input.pop();
                self.update_filtered_files();
            }
            _ => {}
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
            "help" => "Commands: help, vars, stack, break <n>, clear <n>, goto, files".to_string(),
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
                    let prefix = if i == self.call_stack_cursor {
                        ">"
                    } else {
                        " "
                    };
                    format!("{} {} (line {})", prefix, f.name, f.line)
                })
                .collect::<Vec<_>>()
                .join("\n"),
            "goto" => {
                self.code_cursor_line = self.current_line;
                format!("Jumped to current execution line {}", self.current_line)
            }
            "files" => self.get_file_list().join(", "),
            other if other.starts_with("break ") => {
                if let Ok(n) = other[6..].trim().parse::<usize>() {
                    if !self.breakpoints.iter().any(|bp| bp.line == n) {
                        self.breakpoints.push(Breakpoint {
                            line: n,
                            enabled: true,
                        });
                        self.breakpoints.sort_by_key(|bp| bp.line);
                        format!("Breakpoint set at line {}", n)
                    } else {
                        format!("Breakpoint already exists at line {}", n)
                    }
                } else {
                    "Usage: break <line_number>".to_string()
                }
            }
            other if other.starts_with("clear ") => {
                if let Ok(n) = other[6..].trim().parse::<usize>() {
                    if let Some(idx) = self.breakpoints.iter().position(|bp| bp.line == n) {
                        self.breakpoints.remove(idx);
                        format!("Breakpoint cleared at line {}", n)
                    } else {
                        format!("No breakpoint at line {}", n)
                    }
                } else {
                    "Usage: clear <line_number>".to_string()
                }
            }
            other if other.starts_with("open ") => {
                let filename = other[5..].trim();
                if self.files.contains_key(filename) {
                    self.current_file = self.files.keys().find(|&&k| k == filename).unwrap();
                    self.code_cursor_line = 1;
                    format!("Opened file: {}", filename)
                } else {
                    format!(
                        "File not found: {}. Available: {}",
                        filename,
                        self.get_file_list().join(", ")
                    )
                }
            }
            _ => format!("Unknown command: {}", cmd),
        };
        self.state_output.push(response);
        self.state_input.clear();
    }

    fn step_into(&mut self) {
        self.debug_state = DebugState::Stopped;
        let code_lines = self.get_current_file_lines();
        let line = code_lines.get(self.current_line.saturating_sub(1));

        if let Some(code) = line {
            if code.contains("process(") {
                self.call_stack.push(StackFrame {
                    name: "process",
                    line: self.current_line,
                });
                self.current_line = 8;
                self.code_cursor_line = 8;
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
                self.code_cursor_line = 14;
                self.variables.push(("n", "10".to_string()));
                self.state_output.push("Step into: validate()".to_string());
                return;
            }
        }

        self.advance_line();
        self.code_cursor_line = self.current_line;
        self.state_output
            .push(format!("Step into: line {}", self.current_line));
    }

    fn step_out(&mut self) {
        self.debug_state = DebugState::Stopped;
        if self.call_stack.len() > 1 {
            if let Some(frame) = self.call_stack.pop() {
                self.current_line = frame.line;
                self.advance_line();
                self.code_cursor_line = self.current_line;
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
        self.code_cursor_line = self.current_line;
        self.update_variables_for_line();
        self.state_output
            .push(format!("Step over: line {}", self.current_line));
    }

    fn continue_execution(&mut self) {
        self.debug_state = DebugState::Running;
        self.state_output.push("Continuing...".to_string());

        let start = self.current_line;
        let code_len = self.get_current_file_lines().len();
        loop {
            self.advance_line();
            self.update_variables_for_line();

            // Check if we hit an enabled breakpoint
            if self
                .breakpoints
                .iter()
                .any(|bp| bp.line == self.current_line && bp.enabled)
            {
                self.debug_state = DebugState::Stopped;
                self.code_cursor_line = self.current_line;
                self.state_output
                    .push(format!("Hit breakpoint at line {}", self.current_line));
                return;
            }

            if self.current_line == start || self.current_line >= code_len {
                self.debug_state = DebugState::Stopped;
                self.code_cursor_line = self.current_line;
                self.state_output
                    .push("No more breakpoints hit".to_string());
                return;
            }
        }
    }

    fn advance_line(&mut self) {
        let code_len = self.get_current_file_lines().len();
        let current_code = self
            .get_current_file_lines()
            .get(self.current_line.saturating_sub(1))
            .copied()
            .unwrap_or("");

        if current_code.trim() == "}" {
            if self.call_stack.len() > 1 {
                if let Some(frame) = self.call_stack.pop() {
                    self.current_line = frame.line + 1;
                    return;
                }
            }
        }

        self.current_line += 1;
        if self.current_line > code_len {
            self.current_line = 1;
        }

        let new_code = self
            .get_current_file_lines()
            .get(self.current_line.saturating_sub(1))
            .copied()
            .unwrap_or("");
        if new_code.trim().is_empty()
            || new_code.trim().starts_with("fn ")
            || new_code.trim() == "}"
        {
            let code_len = self.get_current_file_lines().len();
            if self.current_line < code_len {
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
        // Draw command palette overlay if open
        if self.command_palette_open {
            self.draw_with_command_palette(frame);
            return;
        }

        let outer = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(8),
        ])
        .split(frame.area());

        let middle =
            Layout::horizontal([Constraint::Length(22), Constraint::Min(40)]).split(outer[1]);

        // Split the left panel into two: Call Stack and Breakpoints
        let left_sections =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(middle[0]);

        self.render_header(frame, outer[0]);
        self.render_call_stack_panel(frame, left_sections[0]);
        self.render_breakpoints_panel(frame, left_sections[1]);
        self.render_code_window(frame, middle[1]);
        self.render_bottom_panel(frame, outer[2]);
    }

    fn draw_with_command_palette(&self, frame: &mut Frame) {
        // Draw normal UI first
        let outer = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(8),
        ])
        .split(frame.area());

        let middle =
            Layout::horizontal([Constraint::Length(22), Constraint::Min(40)]).split(outer[1]);

        let left_sections =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(middle[0]);

        self.render_header(frame, outer[0]);
        self.render_call_stack_panel(frame, left_sections[0]);
        self.render_breakpoints_panel(frame, left_sections[1]);
        self.render_code_window(frame, middle[1]);
        self.render_bottom_panel(frame, outer[2]);

        // Draw command palette overlay
        let area = frame.area();
        let palette_width = 40.min(area.width.saturating_sub(4));
        let palette_height = (self.command_palette_filtered.len() as u16 + 3).min(12);
        let palette_x = (area.width.saturating_sub(palette_width)) / 2;
        let palette_y = 3;

        let palette_area = Rect::new(palette_x, palette_y, palette_width, palette_height);

        // Clear background
        let bg = Paragraph::new("").style(Style::default().bg(Color::Black));
        frame.render_widget(bg, palette_area);

        // Draw border and content
        let mut lines = vec![
            Line::from(vec![
                Span::styled(" Open File ", Style::default().bold()),
                Span::raw("(Ctrl+P to close)"),
            ]),
            Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Green)),
                Span::raw(&self.command_palette_input),
                Span::styled("_", Style::default().add_modifier(Modifier::SLOW_BLINK)),
            ]),
            Line::from("-".repeat(palette_width as usize - 2)),
        ];

        for (i, &file) in self.command_palette_filtered.iter().enumerate() {
            let style = if i == self.command_palette_cursor {
                Style::default().bg(Color::DarkGray).bold()
            } else {
                Style::default()
            };
            let prefix = if i == self.command_palette_cursor {
                "> "
            } else {
                "  "
            };
            lines.push(Line::from(Span::styled(
                format!("{}{}", prefix, file),
                style,
            )));
        }

        frame.render_widget(
            Paragraph::new(lines).style(Style::default().bg(Color::Black)),
            palette_area,
        );
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let (status, status_color) = match self.debug_state {
            DebugState::Stopped => ("[STOPPED]", Color::Yellow),
            DebugState::Running => ("[RUNNING]", Color::Green),
        };
        let title = format!("Debugger - {}", self.current_file);
        let keys = "F7:into S-F7:out F8:over F9:cont C-p:files";
        let padding = area
            .width
            .saturating_sub((title.len() + status.len() + keys.len() + 4) as u16)
            as usize;
        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled(&title, Style::default().bold()),
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

    fn render_call_stack_panel(&self, frame: &mut Frame, area: Rect) {
        let focused = self.focus == PanelFocus::CallStack;
        let style = if focused {
            Style::default()
        } else {
            Style::default().dim()
        };

        let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(2)]).split(area);

        let title_style = if focused {
            Style::default().bold().fg(Color::Cyan)
        } else {
            Style::default().dim()
        };
        let title = Line::from(vec![
            Span::styled("Call Stack", title_style),
            Span::raw(if focused { " [j/k/Enter]" } else { "" }),
        ]);
        frame.render_widget(Paragraph::new(title), chunks[0]);

        let cs_lines: Vec<Line> = self
            .call_stack
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let is_selected = i == self.call_stack_cursor;
                let prefix = if is_selected { "> " } else { "  " };
                let line_style = if is_selected && focused {
                    Style::default().bg(Color::DarkGray).bold()
                } else {
                    Style::default()
                };
                Line::from(Span::styled(
                    format!("{}{}:{}", prefix, f.name, f.line),
                    line_style,
                ))
            })
            .collect();
        frame.render_widget(Paragraph::new(cs_lines).style(style), chunks[1]);
    }

    fn render_breakpoints_panel(&self, frame: &mut Frame, area: Rect) {
        let focused = self.focus == PanelFocus::Breakpoints;
        let style = if focused {
            Style::default()
        } else {
            Style::default().dim()
        };

        let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(2)]).split(area);

        let title_style = if focused {
            Style::default().bold().fg(Color::Cyan)
        } else {
            Style::default().dim()
        };

        // Show add breakpoint prompt if active
        if self.adding_breakpoint {
            let title = Line::from(vec![
                Span::styled("Add BP line: ", title_style),
                Span::raw(&self.new_breakpoint_input),
                Span::styled("_", Style::default().add_modifier(Modifier::SLOW_BLINK)),
            ]);
            frame.render_widget(Paragraph::new(title), chunks[0]);
        } else {
            let title = Line::from(vec![
                Span::styled("Breakpoints", title_style),
                Span::raw(if focused { " [j/k/Space/a/d]" } else { "" }),
            ]);
            frame.render_widget(Paragraph::new(title), chunks[0]);
        }

        let mut bp_list: Vec<&Breakpoint> = self.breakpoints.iter().collect();
        bp_list.sort_by_key(|bp| bp.line);

        let bp_lines: Vec<Line> = bp_list
            .iter()
            .enumerate()
            .map(|(i, bp)| {
                let is_selected = i == self.breakpoint_cursor;
                let prefix = if is_selected { "> " } else { "  " };
                let marker_color = if bp.enabled {
                    Color::Red
                } else {
                    Color::DarkGray
                };
                let marker = if bp.enabled { "[x]" } else { "[ ]" };
                let line_style = if is_selected && focused {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };
                Line::from(vec![
                    Span::styled(prefix, line_style),
                    Span::styled(marker, Style::default().fg(marker_color)),
                    Span::styled(format!(" line {}", bp.line), line_style),
                ])
            })
            .collect();

        if bp_lines.is_empty() {
            frame.render_widget(
                Paragraph::new("  (no breakpoints)").style(style.dim()),
                chunks[1],
            );
        } else {
            frame.render_widget(Paragraph::new(bp_lines).style(style), chunks[1]);
        }
    }

    fn render_code_window(&self, frame: &mut Frame, area: Rect) {
        let focused = self.focus == PanelFocus::CodeWindow;
        let style = if focused {
            Style::default()
        } else {
            Style::default().dim()
        };

        let code_lines = self.get_current_file_lines();
        let lines: Vec<Line> = code_lines
            .iter()
            .enumerate()
            .map(|(idx, &code)| {
                let line_num = idx + 1;
                let is_current_exec = line_num == self.current_line;
                let is_cursor = line_num == self.code_cursor_line && focused;
                let has_breakpoint = self.breakpoints.iter().any(|bp| bp.line == line_num);

                let bp_marker = if has_breakpoint {
                    let bp = self.breakpoints.iter().find(|bp| bp.line == line_num);
                    let color = if bp.map_or(true, |b| b.enabled) {
                        Color::Red
                    } else {
                        Color::DarkGray
                    };
                    Span::styled("*", Style::default().fg(color))
                } else {
                    Span::raw(" ")
                };

                let exec_indicator = if is_current_exec { ">>" } else { "  " };

                let num_span = Span::styled(
                    format!("{:3}", line_num),
                    Style::default().fg(Color::DarkGray),
                );

                // Determine code style based on execution position and cursor
                let code_style = if is_current_exec && is_cursor {
                    Style::default()
                        .bg(Color::Yellow)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD)
                } else if is_current_exec {
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else if is_cursor {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else {
                    Style::default()
                };

                Line::from(vec![
                    bp_marker,
                    Span::raw(" "),
                    Span::styled(exec_indicator, Style::default().fg(Color::Yellow)),
                    Span::raw(" "),
                    num_span,
                    Span::raw(" | "),
                    Span::styled(code.to_string(), code_style),
                ])
            })
            .collect();

        // Add help line at bottom when focused
        let mut all_lines = lines;
        if focused {
            all_lines.push(Line::from(""));
            all_lines.push(Line::from(Span::styled(
                " j/k:move g/G:top/bottom b:toggle-bp 0:goto-exec {/}:para H/L:page",
                Style::default().dim(),
            )));
        }

        frame.render_widget(Paragraph::new(all_lines).style(style), area);
    }

    fn render_bottom_panel(&self, frame: &mut Frame, area: Rect) {
        let focused = self.focus == PanelFocus::BottomPanel;
        let style = if focused {
            Style::default()
        } else {
            Style::default().dim()
        };

        let separator = "-".repeat(area.width as usize);
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
