use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub enum DebugStatus {
    Paused,
    Running,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StackFrame {
    pub name: String,
    pub file: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variable {
    pub name: String,
    pub value: String,
    pub var_type: String,
    pub children: Vec<Variable>,
    pub expanded: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DebuggerState {
    pub status: DebugStatus,
    pub current_line: usize,
    pub source_lines: Vec<String>,
    pub breakpoints: HashSet<usize>,
    pub stack_frames: Vec<StackFrame>,
    pub selected_frame: usize,
    pub variables: Vec<Variable>,
    pub console_output: Vec<String>,
}

impl DebuggerState {
    pub fn toggle_breakpoint(&mut self, line: usize) {
        if !self.breakpoints.remove(&line) {
            self.breakpoints.insert(line);
        }
    }

    pub fn select_frame(&mut self, index: usize) {
        if index < self.stack_frames.len() {
            self.selected_frame = index;
        }
    }

    pub fn step_over(&mut self) {
        if self.status != DebugStatus::Paused {
            return;
        }
        // Advance to next non-blank line
        let mut next = self.current_line + 1;
        while next <= self.source_lines.len() {
            if let Some(line) = self.source_lines.get(next - 1) {
                if !line.trim().is_empty() {
                    break;
                }
            }
            next += 1;
        }
        if next <= self.source_lines.len() {
            self.current_line = next;
            self.console_output.push(format!("Stepped to line {next}"));
        }
    }

    pub fn step_in(&mut self) {
        if self.status != DebugStatus::Paused {
            return;
        }
        // Mock: push a new frame and jump into process_data at line 11
        self.stack_frames.insert(
            0,
            StackFrame {
                name: "process_data".to_string(),
                file: "example.py".to_string(),
                line: 11,
            },
        );
        self.selected_frame = 0;
        self.current_line = 11;
        self.console_output
            .push("Stepped into process_data".to_string());
    }

    pub fn step_out(&mut self) {
        if self.status != DebugStatus::Paused {
            return;
        }
        if self.stack_frames.len() > 1 {
            let popped = self.stack_frames.remove(0);
            self.selected_frame = 0;
            self.current_line = self.stack_frames[0].line;
            self.console_output
                .push(format!("Stepped out of {}", popped.name));
        }
    }

    pub fn continue_running(&mut self) {
        if self.status != DebugStatus::Paused {
            return;
        }
        // Jump to the next breakpoint after current_line
        let next_bp = self
            .breakpoints
            .iter()
            .filter(|&&line| line > self.current_line)
            .min()
            .copied();

        if let Some(line) = next_bp {
            self.current_line = line;
            self.console_output
                .push(format!("Hit breakpoint at line {line}"));
        } else {
            self.status = DebugStatus::Running;
            self.console_output.push("Running...".to_string());
        }
    }

    pub fn toggle_variable_expanded(&mut self, path: &[usize]) {
        if let Some(var) = Self::variable_at_path(&mut self.variables, path) {
            var.expanded = !var.expanded;
        }
    }

    fn variable_at_path<'a>(vars: &'a mut [Variable], path: &[usize]) -> Option<&'a mut Variable> {
        match path {
            [] => None,
            [idx] => vars.get_mut(*idx),
            [idx, rest @ ..] => {
                let var = vars.get_mut(*idx)?;
                Self::variable_at_path(&mut var.children, rest)
            }
        }
    }
}
