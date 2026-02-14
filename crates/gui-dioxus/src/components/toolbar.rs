use dioxus::prelude::*;

use crate::state::{DebugStatus, DebuggerState};

#[component]
pub fn Toolbar(state: Signal<DebuggerState>) -> Element {
    let is_paused = state.read().status == DebugStatus::Paused;

    rsx! {
        div { class: "toolbar",
            button {
                onclick: move |_| state.write().continue_running(),
                "▶ Continue"
            }
            button {
                onclick: move |_| state.write().step_over(),
                "⏭ Step Over"
            }
            button {
                onclick: move |_| state.write().step_in(),
                "⏬ Step In"
            }
            button {
                onclick: move |_| state.write().step_out(),
                "⏫ Step Out"
            }
            span {
                class: if is_paused { "status-indicator status-paused" } else { "status-indicator status-running" },
                if is_paused { "⏸ Paused" } else { "▶ Running" }
            }
        }
    }
}
