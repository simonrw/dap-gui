use eframe::egui::Widget;

pub(crate) struct Breakpoints<'s> {
    breakpoints: &'s [debugger::Breakpoint],
    show_details: bool,
}

impl<'s> Breakpoints<'s> {
    pub(crate) fn new(breakpoints: &'s [debugger::Breakpoint], show_details: bool) -> Self {
        Self {
            breakpoints,
            show_details,
        }
    }
}

impl Widget for Breakpoints<'_> {
    fn ui(self, ui: &mut eframe::egui::Ui) -> eframe::egui::Response {
        let mut final_response = ui.label("Breakpoints");
        if self.show_details {
            for breakpoint in self.breakpoints {
                if let Some(name) = &breakpoint.name {
                    final_response |= ui.label(format!(
                        "{path}:{line} ({name})",
                        path = breakpoint.path.display(),
                        line = breakpoint.line,
                        name = name
                    ));
                } else {
                    final_response |= ui.label(format!(
                        "{path}:{line}",
                        path = breakpoint.path.display(),
                        line = breakpoint.line,
                    ));
                }
            }
        }
        final_response
    }
}
