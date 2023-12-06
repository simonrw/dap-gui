#[derive(Debug, Clone)]
pub(crate) enum Message {
    Launch,
    Quit,
    DebuggerMessage(debugger::Event),
    Continue,
    // Temp
    DefineBreakpoint(String),
    AddBreakpoint,
}
