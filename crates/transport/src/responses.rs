//! Responses in reply to [`crate::requests`] from a DAP server
use crate::types::{
    self, Scope, StackFrame, Thread, Variable, VariablePresentationHint, VariablesReference,
};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", content = "body", rename_all = "camelCase")]
#[non_exhaustive]
pub enum ResponseBody {
    Initialize(Capabilities),
    SetFunctionBreakpoints(SetFunctionBreakpointsResponse),
    SetBreakpoints(SetBreakpoints),
    BreakpointLocations(BreakpointLocationsResponse),
    Continue(ContinueResponse),
    Threads(ThreadsResponse),
    StackTrace(StackTraceResponse),
    Scopes(ScopesResponse),
    Variables(VariablesResponse),
    ConfigurationDone,
    Terminate,
    Disconnect,
    Evaluate(EvaluateResponse),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub supports_configuration_done_request: Option<bool>,
    pub supports_function_breakpoints: Option<bool>,
    pub supports_conditional_breakpoints: Option<bool>,
    pub supports_hit_conditional_breakpoints: Option<bool>,
    pub supports_evaluate_for_hovers: Option<bool>,
    // pub exception_breakpoint_filters: Option<Vec<ExceptionBreakpointsFilter>>,
    pub supports_step_back: Option<bool>,
    pub supports_set_variable: Option<bool>,
    pub supports_restart_frame: Option<bool>,
    pub supports_goto_targets_request: Option<bool>,
    pub supports_step_in_targets_request: Option<bool>,
    pub supports_completions_request: Option<bool>,
    pub completion_trigger_characters: Option<Vec<String>>,
    pub supports_modules_request: Option<bool>,
    // pub additional_module_columns: Option<Vec<ColumnDescriptor>>,
    // pub supported_checksum_algorithms: Option<Vec<ChecksumAlgorithm>>,
    pub supports_restart_request: Option<bool>,
    pub supports_exception_options: Option<bool>,
    pub supports_value_formatting_options: Option<bool>,
    pub supports_exception_info_request: Option<bool>,
    pub support_terminate_debuggee: Option<bool>,
    pub support_suspend_debuggee: Option<bool>,
    pub supports_delayed_stack_trace_loading: Option<bool>,
    pub supports_loaded_sources_request: Option<bool>,
    pub supports_log_points: Option<bool>,
    pub supports_terminate_threads_request: Option<bool>,
    pub supports_set_expression: Option<bool>,
    pub supports_terminate_request: Option<bool>,
    pub supports_data_breakpoints: Option<bool>,
    pub supports_read_memory_request: Option<bool>,
    pub supports_write_memory_request: Option<bool>,
    pub supports_disassemble_request: Option<bool>,
    pub supports_cancel_request: Option<bool>,
    pub supports_breakpoint_locations_request: Option<bool>,
    pub supports_clipboard_context: Option<bool>,
    pub supports_stepping_granularity: Option<bool>,
    pub supports_instruction_breakpoints: Option<bool>,
    pub supports_exception_filter_options: Option<bool>,
    pub supports_single_thread_execution_requests: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetFunctionBreakpointsResponse {
    pub breakpoints: Vec<types::Breakpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetBreakpoints {
    pub breakpoints: Vec<types::Breakpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BreakpointLocationsResponse {
    pub breakpoints: Vec<types::BreakpointLocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueResponse {
    pub all_threads_continued: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadsResponse {
    pub threads: Vec<Thread>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackTraceResponse {
    pub stack_frames: Vec<StackFrame>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopesResponse {
    pub scopes: Vec<Scope>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariablesResponse {
    pub variables: Vec<Variable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateResponse {
    pub result: String,
    pub r#type: Option<String>,
    pub presentation_hint: Option<VariablePresentationHint>,
    pub variables_reference: VariablesReference,
    pub named_variables: Option<usize>,
    pub indexed_variables: Option<usize>,
    pub memory_reference: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
