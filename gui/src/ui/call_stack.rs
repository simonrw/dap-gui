use eframe::egui::{Response, Widget};
use transport::types::StackFrame;

use crate::DebuggerAppState;

pub(crate) struct CallStack<'s> {
    stack: &'s [StackFrame],
    show_details: bool,
    state: &'s DebuggerAppState,
}

impl<'s> CallStack<'s> {
    pub(crate) fn new(
        stack: &'s [StackFrame],
        show_details: bool,
        state: &'s DebuggerAppState,
    ) -> Self {
        Self {
            stack,
            show_details,
            state,
        }
    }
}

impl<'s> Widget for CallStack<'s> {
    fn ui(self, ui: &mut eframe::egui::Ui) -> Response {
        let final_response = ui.heading("Call Stack");

        if self.show_details {
            for frame in self.stack {
                if ui.link(frame.name.to_string()).clicked() {
                    if let Err(e) = self.state.change_scope(frame.id) {
                        tracing::warn!(error = ?e, "error changing scope");
                    }
                }
            }
        }

        final_response
    }
}
