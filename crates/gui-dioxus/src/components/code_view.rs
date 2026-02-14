use dioxus::prelude::*;

use crate::state::DebuggerState;

#[component]
pub fn CodeView(state: Signal<DebuggerState>) -> Element {
    let current_line = state.read().current_line;
    let lines = state.read().source_lines.clone();
    let breakpoints = state.read().breakpoints.clone();

    rsx! {
        div { class: "code-view",
            for (i, line) in lines.iter().enumerate() {
                {
                    let line_num = i + 1;
                    let is_current = line_num == current_line;
                    let has_bp = breakpoints.contains(&line_num);
                    let line_text = line.clone();

                    rsx! {
                        div {
                            class: if is_current { "code-line current-line" } else { "code-line" },
                            div {
                                class: "gutter",
                                onclick: move |_| state.write().toggle_breakpoint(line_num),
                                span { class: "line-number", "{line_num}" }
                                span {
                                    class: if has_bp { "bp-slot bp-active" } else { "bp-slot" },
                                    "●"
                                }
                            }
                            span {
                                class: "current-arrow",
                                if is_current { "→" } else { " " }
                            }
                            span { class: "code-text", "{line_text}" }
                        }
                    }
                }
            }
        }
    }
}
