//! General types used common to [`crate::requests`], [`crate::responses`] or [`crate::events`].
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub type Seq = i64;
pub type ThreadId = i64;
pub type BreakpointId = i64;
pub type StackFrameId = i64;
pub type VariablesReference = i64;
pub type SourceReference = i64;

#[derive(Deserialize, Debug, Clone)]
pub struct Thread {
    pub id: ThreadId,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PresentationHint {
    Arguments,
    Locals,
    Registers,
    Other(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StackFrameFormat {
    /// Displays parameters for the stack frame
    pub parameters: Option<bool>,
    /// Displays the types of parameters for the stack frame
    parameter_types: Option<bool>,
    /// Displays the names of parameters for the stack frame
    pub parameter_names: Option<bool>,
    /// Displays the values of parameters for the stack frame
    pub parameter_values: Option<bool>,
    /// Displays the line number of the stack frame
    pub line: Option<bool>,
    /// Displays the module of the stack frame
    pub module: Option<bool>,
    /// Includes all stack frames, including those the debug adapter might otherwise hide
    pub include_all: Option<bool>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Scope {
    pub name: String,
    pub variables_reference: VariablesReference,
    pub presentation_hint: Option<PresentationHint>,
    pub named_variables: Option<usize>,
    pub indexed_variables: Option<usize>,
    pub expensive: bool,
    pub line: Option<i64>,
    pub column: Option<i64>,
    pub source: Option<Source>,
    pub end_line: Option<i64>,
    pub end_column: Option<i64>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct Source {
    pub name: Option<String>,
    pub path: Option<PathBuf>,
    #[serde(rename = "sourceReference")]
    pub source_reference: Option<SourceReference>,
    #[serde(rename = "presentationHint")]
    pub presentation_hint: Option<String>,
    pub origin: Option<String>,
    pub sources: Option<Vec<Source>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Breakpoint {
    pub id: Option<BreakpointId>,
    pub verified: bool,
    pub message: Option<String>,
    pub source: Option<Source>,
    pub line: Option<i64>,
    pub column: Option<i64>,
    pub end_line: Option<i64>,
    pub end_column: Option<i64>,
    pub instruction_reference: Option<String>,
    pub offset: Option<i64>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SourceBreakpoint {
    /// The source line of the breakpoint or logpoint.
    pub line: usize,
    /// Start position within source line of the breakpoint or logpoint. It is measured in UTF-16
    /// code units and the client capability `columnsStartAt1` determines whether it is 0- or
    /// 1-based.
    pub column: Option<usize>,
    /// The expression for conditional breakpoints. It is only honored by a debug adapter if the
    /// corresponding capability `supportsConditionalBreakpoints` is true.
    pub condition: Option<String>,
    /// The expression that controls how many hits of the breakpoint are ignored. The debug adapter
    /// is expected to interpret the expression as needed. The attribute is only honored by a debug
    /// adapter if the corresponding capability `supportsHitConditionalBreakpoints` is true. If
    /// both this property and `condition` are specified, `hitCondition` should be evaluated only
    /// if the `condition` is met, and the debug adapter should stop only if both conditions are
    /// met.
    pub hit_condition: Option<String>,
    /// If this attribute exists and is non-empty, the debug adapter must not 'break' (stop) but
    /// log the message instead. Expressions within `{}` are interpolated. The attribute is only
    /// honored by a debug adapter if the corresponding capability `supportsLogPoints` is true. If
    /// either `hitCondition` or `condition` is specified, then the message should only be logged
    /// if those conditions are met.
    pub log_message: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StackFrame {
    pub id: StackFrameId,
    pub name: String,
    pub source: Option<Source>,
    pub line: usize,
    pub column: usize,
    pub end_line: Option<usize>,
    pub end_column: Option<usize>,
    pub can_restart: Option<bool>,
    pub module_id: Option<ModuleId>,
    pub presentation_hint: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VariablePresentationHint {
    pub kind: Option<String>,
    pub attributes: Option<String>,
    pub visibility: Option<String>,
    pub lazy: Option<bool>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub name: String,
    pub value: String,
    pub r#type: Option<String>,
    pub variables_reference: VariablesReference,
    pub presentation_hint: Option<VariablePresentationHint>,
}

#[derive(Deserialize, Debug, Clone)]
pub enum ModuleId {
    Number(i64),
    String(String),
}

#[derive(Deserialize, Debug, Clone)]
pub struct Module {
    pub id: i64,
    pub name: String,
    pub path: Option<PathBuf>,
}
