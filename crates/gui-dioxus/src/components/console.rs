use dioxus::prelude::*;

use crate::state::DebuggerState;

#[component]
pub fn Console(state: Signal<DebuggerState>) -> Element {
    let output = state.read().console_output.clone();

    rsx! {
        div { class: "console",
            div { class: "panel-header", "Console" }
            div { class: "console-output",
                for line in output.iter() {
                    div { class: "console-line", "{line}" }
                }
            }
        }
    }
}
