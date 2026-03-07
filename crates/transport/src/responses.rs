//! Responses in reply to [`crate::requests`] from a DAP server
use serde::{Deserialize, Serialize};

pub use dap_types::{
    Capabilities, ContinueBody, EvaluateBody, ResponseBody, ScopesBody, SetBreakpointsBody,
    SetFunctionBreakpointsBody, StackTraceBody, ThreadsBody, VariablesBody,
};

// Backwards-compatible type aliases for old names
pub type BreakpointLocationsResponse = dap_types::BreakpointLocationsBody;
pub type ContinueResponse = ContinueBody;
pub type EvaluateResponse = EvaluateBody;
pub type ScopesResponse = ScopesBody;
pub type SetBreakpoints = SetBreakpointsBody;
pub type SetFunctionBreakpointsResponse = SetFunctionBreakpointsBody;
pub type StackTraceResponse = StackTraceBody;
pub type ThreadsResponse = ThreadsBody;
pub type VariablesResponse = VariablesBody;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    #[serde(rename = "request_seq")]
    pub request_seq: i64,
    pub success: bool,
    pub message: Option<String>,
    #[serde(flatten)]
    pub body: Option<ResponseBody>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Variable;

    #[test]
    fn variables_response() {
        let response = r#"
    {
        "variables": [
            {
                "evaluateName": "foo",
                "name": "return",
                "presentationHint": {
                    "attributes": ["readOnly"],
                    "type": "NoneType",
                    "value": "None",
                    "variablesReference": 0
                }
            },
            {
                "evaluateName": "b",
                "name": "b",
                "type": "int",
                "value": "20",
                "variablesReference": 0
            },
            {
                "evaluateName": "c",
                "name": "c",
                "type": "Bar",
                "value": "<__main__.Bar>",
                "variablesReference": 6 }
        ]
    }
        "#;

        let _response: VariablesResponse = serde_json::from_str(response).unwrap();
    }

    #[test]
    fn response2() {
        let responses = [
            r#"
            {
                "name": "(return) Bar.__init__",
                "value": "None",
                "evaluateName": "__pydevd_ret_val_dict['Bar.__init__']",
                "type": "NoneType",
                "variablesReference": 0,
                "presentationHint": {
                    "kind": null,
                    "attributes": ["readOnly"],
                    "visibility": null,
                    "lazy": null
                }
            }
        "#,
            r#"{
                "name": "b",
                "value": "20",
                "evaluateName": "b",
                "type": "int",
                "variablesReference": 0,
                "presentationHint": null
            }"#,
            r#"{
                "name": "c",
                "value": "<__main__.Bar object at 0x1075c9010>",
                "evaluateName": "c",
                "type": "Bar",
                "variablesReference": 6,
                "presentationHint": null
            }"#,
            r#"{
                "name": "(return) foo",
                "value": "5",
                "evaluateName": "__pydevd_ret_val_dict['foo']",
                "type": "int",
                "variablesReference": 0,
                "presentationHint": {
                    "kind": null,
                    "attributes": ["readOnly"],
                    "visibility": null,
                    "lazy": null
                }
            }"#,
        ];

        for response in responses {
            let _response: Variable = serde_json::from_str(response).expect(response);
        }
    }
}
