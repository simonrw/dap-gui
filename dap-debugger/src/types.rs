use dap_gui_client::requests;

/// Manage the user facing types of the debugger.
///
/// Note: these types are not used as part of the debug protocol itself.

/// Defines the location of a breakpoint
pub struct Breakpoint {
    pub source: String,
    pub line_number: u64,
}

/// Defines a breakpoint associated with a function
pub struct FunctionBreakpoint {
    pub name: String,
}

impl From<FunctionBreakpoint> for requests::FunctionBreakpoint {
    fn from(value: FunctionBreakpoint) -> Self {
        requests::FunctionBreakpoint { name: value.name }
    }
}
