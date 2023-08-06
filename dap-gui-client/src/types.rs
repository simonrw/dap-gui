use serde::Deserialize;

pub type ThreadId = i64;
pub type StackFrameId = i64;
pub type VariablesReference = i64;
pub type SourceReference = i64;

#[derive(Deserialize, Debug, Clone)]
pub struct Thread {
    pub id: ThreadId,
    pub name: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Scope {
    pub name: String,
    #[serde(rename = "variablesReference")]
    pub variables_reference: i64,
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
    pub id: Option<i64>,
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
pub struct Variable {
    pub name: String,
    pub value: String,
    pub r#type: Option<String>,
    #[serde(rename = "variablesReference")]
    pub variables_reference: VariablesReference,
}
