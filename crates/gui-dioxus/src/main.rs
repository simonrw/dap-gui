use dioxus::prelude::*;

mod components;
mod mock;
mod state;

use components::{CallStack, CodeView, Console, Toolbar, Variables};

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let state = use_signal(mock::default_state);
    let mut split_y = use_signal(|| 250.0_f64);
    let mut dragging = use_signal(|| false);

    let main_class = if dragging() { "dragging" } else { "" };

    rsx! {
        document::Link { rel: "stylesheet", href: asset!("assets/style.css") }
        div {
            id: "main",
            class: main_class,
            onmousemove: move |e: MouseEvent| {
                if dragging() {
                    let y = e.page_coordinates().y;
                    let new_split = (y - 40.0).clamp(60.0, 2000.0);
                    split_y.set(new_split);
                }
            },
            onmouseup: move |_| dragging.set(false),
            Toolbar { state }
            div { class: "sidebar",
                div {
                    class: "sidebar-top",
                    style: "height: {split_y()}px",
                    CallStack { state }
                }
                div {
                    class: "sidebar-separator",
                    onmousedown: move |e: MouseEvent| {
                        e.prevent_default();
                        dragging.set(true);
                    },
                }
                div { class: "sidebar-bottom",
                    Variables { state }
                }
            }
            CodeView { state }
            Console { state }
        }
    }
}
