use dioxus::prelude::*;

use crate::state::DebuggerState;

#[component]
pub fn CallStack(state: Signal<DebuggerState>) -> Element {
    let frames = state.read().stack_frames.clone();
    let selected = state.read().selected_frame;

    rsx! {
        div { class: "call-stack",
            div { class: "panel-header", "Call Stack" }
            for (i, frame) in frames.iter().enumerate() {
                {
                    let name = frame.name.clone();
                    let location = format!("{}:{}", frame.file, frame.line);
                    let is_selected = i == selected;

                    rsx! {
                        div {
                            class: if is_selected { "frame-item selected" } else { "frame-item" },
                            onclick: move |_| state.write().select_frame(i),
                            span { class: "frame-name", "{name}" }
                            span { class: "frame-location", "{location}" }
                        }
                    }
                }
            }
        }
    }
}
