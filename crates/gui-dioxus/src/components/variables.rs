use dioxus::prelude::*;

use crate::state::{DebuggerState, Variable};

#[component]
pub fn Variables(state: Signal<DebuggerState>) -> Element {
    let variables = state.read().variables.clone();

    rsx! {
        div { class: "variables",
            div { class: "panel-header", "Variables" }
            for (i, var) in variables.iter().enumerate() {
                { rsx! { VariableRow { state, var: var.clone(), path: vec![i] } } }
            }
        }
    }
}

#[component]
fn VariableRow(state: Signal<DebuggerState>, var: Variable, path: Vec<usize>) -> Element {
    let has_children = !var.children.is_empty();
    let is_expanded = var.expanded;
    let children = var.children.clone();

    rsx! {
        div {
            div { class: "var-row",
                span {
                    class: "var-toggle",
                    onclick: {
                        let path = path.clone();
                        move |_| state.write().toggle_variable_expanded(&path)
                    },
                    if has_children {
                        if is_expanded { "▼" } else { "▶" }
                    }
                }
                span { class: "var-name", "{var.name}" }
                span { class: "var-separator", "=" }
                span { class: "var-value", "{var.value}" }
                span { class: "var-type", "{var.var_type}" }
            }
            if has_children && is_expanded {
                div { class: "var-children",
                    for (i, child) in children.iter().enumerate() {
                        {
                            let mut child_path = path.clone();
                            child_path.push(i);
                            rsx! { VariableRow { state, var: child.clone(), path: child_path } }
                        }
                    }
                }
            }
        }
    }
}
