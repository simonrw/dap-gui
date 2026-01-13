use std::path::PathBuf;

/// Messages that drive the application state machine.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Message {
    // Debugger commands
    Continue,
    StepOver,
    StepIn,
    StepOut,
    Stop,

    // Breakpoint management
    ToggleBreakpoint(usize), // line number (1-indexed)

    // Debugger lifecycle
    DebuggerConnected,
    DebuggerEvent(debugger::Event),
    CommandResult(Result<(), String>),

    // Source file loading
    SourceLoaded(Result<String, String>),
    LoadSource(PathBuf),
}
