use std::collections::HashSet;
use std::path::PathBuf;

/// Main application state.
pub struct AppState {
    // Debugger connection
    pub connected: bool,
    pub is_running: bool,

    // Source view
    pub current_file: Option<PathBuf>,
    pub source_content: String,
    pub current_line: Option<usize>,

    // Breakpoints (line numbers, 1-indexed)
    pub breakpoints: HashSet<usize>,

    // Placeholders for future panels
    pub stack_frames: Vec<StackFrame>,
    pub variables: Vec<Variable>,
    pub console_output: Vec<String>,
}

/// A stack frame from the debugger.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct StackFrame {
    pub name: String,
    pub file: String,
    pub line: usize,
}

/// A variable from the debugger.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Variable {
    pub name: String,
    pub value: String,
    pub var_type: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            connected: false,
            is_running: false,
            current_file: None,
            source_content: String::new(),
            current_line: None,
            breakpoints: HashSet::new(),
            stack_frames: Vec::new(),
            variables: Vec::new(),
            console_output: Vec::new(),
        }
    }
}
