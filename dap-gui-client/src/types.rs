use serde::Deserialize;

pub type ThreadId = i64;
pub type StackFrameId = i64;

#[derive(Deserialize, Debug, Clone)]
pub struct Thread {
    pub id: ThreadId,
    pub name: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Scope {
    pub name: String,
    #[serde(rename = "variablesReference")]
    pub variables_reference: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Breakpoint {
    pub id: Option<i64>,
    pub verified: bool,
    pub message: Option<String>,
    // pub source: Option<Source>,
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
