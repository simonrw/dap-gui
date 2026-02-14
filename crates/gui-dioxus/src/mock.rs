use std::collections::HashSet;

use crate::state::{DebugStatus, DebuggerState, StackFrame, Variable};

const MOCK_SOURCE: &str = r#"def main():
    print('Hello, debugger!')
    x = 42
    name = 'Alice'
    items = [1, 2, 3, 4, 5]
    result = process_data(items)
    print(f'Result: {result}')
    return result

def process_data(data):
    total = 0
    for item in data:
        total += item
    return total"#;

pub fn default_state() -> DebuggerState {
    DebuggerState {
        status: DebugStatus::Paused,
        current_line: 6,
        source_lines: MOCK_SOURCE.lines().map(String::from).collect(),
        breakpoints: HashSet::from([6, 11]),
        stack_frames: vec![
            StackFrame {
                name: "main".to_string(),
                file: "example.py".to_string(),
                line: 6,
            },
            StackFrame {
                name: "<module>".to_string(),
                file: "example.py".to_string(),
                line: 1,
            },
        ],
        selected_frame: 0,
        variables: vec![
            Variable {
                name: "x".to_string(),
                value: "42".to_string(),
                var_type: "int".to_string(),
                children: vec![],
                expanded: false,
            },
            Variable {
                name: "name".to_string(),
                value: "'Alice'".to_string(),
                var_type: "str".to_string(),
                children: vec![],
                expanded: false,
            },
            Variable {
                name: "items".to_string(),
                value: "[1, 2, 3, 4, 5]".to_string(),
                var_type: "list".to_string(),
                children: vec![
                    Variable {
                        name: "0".to_string(),
                        value: "1".to_string(),
                        var_type: "int".to_string(),
                        children: vec![],
                        expanded: false,
                    },
                    Variable {
                        name: "1".to_string(),
                        value: "2".to_string(),
                        var_type: "int".to_string(),
                        children: vec![],
                        expanded: false,
                    },
                    Variable {
                        name: "2".to_string(),
                        value: "3".to_string(),
                        var_type: "int".to_string(),
                        children: vec![],
                        expanded: false,
                    },
                    Variable {
                        name: "3".to_string(),
                        value: "4".to_string(),
                        var_type: "int".to_string(),
                        children: vec![],
                        expanded: false,
                    },
                    Variable {
                        name: "4".to_string(),
                        value: "5".to_string(),
                        var_type: "int".to_string(),
                        children: vec![],
                        expanded: false,
                    },
                ],
                expanded: false,
            },
        ],
        console_output: vec![
            "Debugger started".to_string(),
            "Breakpoint hit at example.py:6".to_string(),
            "Paused in main()".to_string(),
        ],
    }
}
