//! General types used common to [`crate::requests`], [`crate::responses`] or [`crate::events`].
use serde::Deserialize;

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

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PresentationHint {
    Arguments,
    Locals,
    Registers,
    Other(String),
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

#[derive(Deserialize, Debug, Clone)]
pub struct Source {
    pub name: Option<String>,
    pub path: Option<String>,
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

#[derive(Deserialize, Debug, Clone)]
pub struct StackFrame {
    pub id: StackFrameId,
    pub name: String,
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
    pub id: ModuleId,
    pub name: String,
    pub path: Option<String>,
}
