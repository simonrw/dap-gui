use std::path::PathBuf;

use crate::debugger_bridge::DebuggerHandle;

/// Messages that drive the application state machine.
#[derive(Debug, Clone)]
pub enum Message {
    // Debugger commands (from UI buttons)
    Continue,
    StepOver,
    StepIn,
    StepOut,
    Stop,

    // Breakpoint management
    ToggleBreakpoint(usize), // line number (1-indexed)

    // Debugger lifecycle
    StartDebugSession,
    DebugServerStarted(u16),
    DebuggerReady(DebuggerHandle),
    DebuggerEvent(debugger::Event),
    DebuggerError(String),
    DebuggerDisconnected,

    // Source file loading
    SourceLoaded(Result<String, String>),
    LoadSource(PathBuf),
}
