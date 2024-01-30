use eframe::egui::{Response, Widget};
use transport::types::StackFrame;

pub(crate) struct CallStack<'s> {
    stack: &'s [StackFrame],
    show_details: bool,
}

impl<'s> CallStack<'s> {
    pub(crate) fn new(stack: &'s [StackFrame], show_details: bool) -> Self {
        Self {
            stack,
            show_details,
        }
    }
}

impl<'s> Widget for CallStack<'s> {
    fn ui(self, ui: &mut eframe::egui::Ui) -> Response {
        let mut final_response = ui.label("Call Stack");

        if self.show_details {
            for frame in self.stack {
                final_response |= ui.label(frame.name.to_string());
            }
        }

        final_response
    }
}
